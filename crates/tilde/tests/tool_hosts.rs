//! Tool hosts across their boundaries: management registration, the dial-in Watch/Respond
//! protocol and the synchronous Lambda invoke, each reached through an agent's tools from an
//! agent's invocation.
mod common;
use common::invocation::{Fixture, all_tools, drain, framed};
use futures::StreamExt;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use tilde::{connections::catalog::Endpoints, tools::Target};
use uuid::Uuid;

fn echo_tool() -> Value {
    json!({"name":"restart","description":"Restart a service.","summary":"Restarted a service",
        "inputSchemaJson":json!({"type":"object","properties":{"service":{"type":"string"}},"required":["service"]}).to_string(),
        "annotations":{"destructive":true}})
}

#[tokio::test]
async fn connected_host_publishes_tools_and_answers_calls_over_its_own_stream() {
    let mut fx = Fixture::new(31, Endpoints::default(), all_tools()).await;
    let agent_tools = fx.chat.tools.clone().unwrap();
    let (host, token) = agent_tools.hosts.register("ops-host", None).await.unwrap();
    let token = token.expect("connected hosts receive a token once");
    assert!(!host.available && host.tools.is_empty());

    // The host process: dial Watch, publish one tool, answer each call through Respond.
    let origin = fx.origin.clone();
    let bearer = token.expose_secret().to_owned();
    let (registered, mut ready) = tokio::sync::mpsc::channel::<()>(1);
    let host_process = tokio::spawn(async move {
        let http = reqwest::Client::new();
        let base = format!("{origin}/tilde.tool_host.v1.ToolHostService");
        let watch = http
            .post(format!("{base}/Watch"))
            .bearer_auth(&bearer)
            .header("content-type", "application/connect+json")
            .header("connect-protocol-version", "1")
            .body(framed(json!({"tools":[echo_tool()]})))
            .send()
            .await
            .unwrap();
        assert!(watch.status().is_success());
        let mut stream = watch.bytes_stream();
        let mut buffer = vec![];
        while let Some(chunk) = stream.next().await {
            buffer.extend_from_slice(&chunk.unwrap());
            for frame in drain(&mut buffer) {
                if frame.get("registered").is_some() {
                    registered.send(()).await.unwrap();
                }
                let Some(call) = frame.get("call") else {
                    continue;
                };
                let input: Value =
                    serde_json::from_str(call["inputJson"].as_str().unwrap()).unwrap();
                let outcome = if input["service"] == "forbidden" {
                    json!({"callId":call["callId"],"error":"That service may not be restarted"})
                } else {
                    json!({"callId":call["callId"],"outputJson":json!({"restarted":input["service"],"tool":call["name"]}).to_string()})
                };
                let done = http
                    .post(format!("{base}/Respond"))
                    .bearer_auth(&bearer)
                    .json(&outcome)
                    .send()
                    .await
                    .unwrap();
                assert!(done.status().is_success());
            }
        }
    });
    ready.recv().await.unwrap();

    let host = agent_tools.hosts.get(host.id).await.unwrap();
    assert!(host.available, "an open stream makes the host available");
    assert_eq!(host.tools[0].name, "restart");
    let source = agent_tools
        .add_source(fx.agent, Target::ToolHost(host.id), &["restart".into()])
        .await
        .unwrap();
    assert_eq!(source.slug, "ops-host");

    let tools = fx.tools().await;
    let tool = tools
        .iter()
        .find(|t| t["name"] == "ops-host.restart")
        .expect("host function listed");
    assert_eq!(tool["summary"], "Restarted a service");
    assert_eq!(tool["annotations"]["destructive"], true);

    let result = fx
        .invoke("ops-host.restart", json!({"service":"billing"}))
        .await;
    let output: Value = serde_json::from_str(
        result["outputJson"]
            .as_str()
            .unwrap_or_else(|| panic!("{result}")),
    )
    .unwrap();
    assert_eq!(output, json!({"restarted":"billing","tool":"restart"}));
    let failed = fx
        .invoke("ops-host.restart", json!({"service":"forbidden"}))
        .await;
    assert_eq!(
        failed["error"]["message"],
        "That service may not be restarted"
    );
    let pg = fx.db.pool.get().await.unwrap();
    let queued: i64 = pg
        .query_one("SELECT COUNT(*) FROM tool_host_calls", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        queued, 0,
        "the queue is transient; chat_tool_calls is the record"
    );
    let statuses: Vec<String> = pg
        .query(
            "SELECT status FROM chat_tool_calls WHERE name='ops-host.restart' ORDER BY created_at",
            &[],
        )
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.get(0))
        .collect();
    assert_eq!(statuses, ["completed", "failed"]);

    // A host that went away is refused instead of hanging its callers, and the next invocation
    // is no longer offered its tools.
    host_process.abort();
    pg.execute(
        "UPDATE tool_hosts SET connected_at=NOW()-INTERVAL '5 minutes'",
        &[],
    )
    .await
    .unwrap();
    assert_eq!(
        fx.invoke("ops-host.restart", json!({"service":"search"}))
            .await["error"]["message"],
        "The tool host is offline"
    );
    fx.next_invocation().await;
    assert!(
        fx.tools()
            .await
            .iter()
            .all(|t| t["name"] != "ops-host.restart")
    );
    // Another host's token cannot complete a call it was never given.
    let (_, other) = agent_tools
        .hosts
        .register("other-host", None)
        .await
        .unwrap();
    let denied = reqwest::Client::new()
        .post(format!(
            "{}/tilde.tool_host.v1.ToolHostService/Respond",
            fx.origin
        ))
        .bearer_auth(other.unwrap().expose_secret())
        .json(&json!({"callId":Uuid::new_v4().to_string(),"outputJson":"{}"}))
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), 404);
    drop(pg);
    fx.close().await;
}

#[tokio::test]
async fn lambda_host_is_listed_and_invoked_synchronously() {
    // The gateway signs Lambda invokes with its own AWS credentials. Set before any thread of
    // this test reads the environment; the other test in this binary never does.
    unsafe {
        std::env::set_var("AWS_ACCESS_KEY_ID", "AKIAFIXTURE");
        std::env::set_var("AWS_SECRET_ACCESS_KEY", "fixture-secret");
    }
    let arn = "arn:aws:lambda:eu-central-1:123456789012:function:tools";
    let lambda = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let lambda_origin = format!("http://{}", lambda.local_addr().unwrap());
    let function = tokio::spawn(async move {
        let app = axum::Router::new().route(
            "/2015-03-31/functions/{arn}/invocations",
            axum::routing::post(
                |axum::extract::Path(function): axum::extract::Path<String>,
                 headers: axum::http::HeaderMap,
                 body: axum::Json<Value>| async move {
                    assert_eq!(headers["x-amz-invocation-type"], "RequestResponse");
                    // A second function needs credentials: it publishes a provider, verifies
                    // an instance's key and reads it from each call.
                    if function.ends_with(":crm") {
                        let key = |frame: &Value| frame["credentials"][0]["value"].clone();
                        return axum::Json(if body.get("listTools").is_some() {
                            json!({"tools":[echo_tool()],"provider":{"id":"lambda-crm","name":"Lambda CRM",
                                "connectionTypes":[{"id":"api_key","name":"API key","static":{"schemaJson":
                                json!({"type":"object","properties":{"api_key":{"type":"string","minLength":1}},
                                    "required":["api_key"],"additionalProperties":false}).to_string()}}]}})
                        } else if let Some(verify) = body.get("verify") {
                            if key(verify) == "lambda-key" { json!({"accountLabel":"ops"}) } else { json!({"error":"Unknown key"}) }
                        } else {
                            json!({"outputJson":json!({"key":key(&body["call"])}).to_string()})
                        });
                    }
                    assert!(
                        headers["authorization"]
                            .to_str()
                            .unwrap()
                            .starts_with("AWS4-HMAC-SHA256")
                    );
                    if body.get("listTools").is_some() {
                        return axum::Json(json!({"tools":[echo_tool()]}));
                    }
                    let input: Value =
                        serde_json::from_str(body["call"]["inputJson"].as_str().unwrap()).unwrap();
                    if input["service"] == "slow" {
                        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
                    }
                    axum::Json(
                        json!({"outputJson":json!({"restarted":input["service"]}).to_string()}),
                    )
                },
            ),
        );
        axum::serve(lambda, app).await.unwrap()
    });
    let spans = common::capture_spans();
    let mut fx = Fixture::new(
        32,
        Endpoints(BTreeMap::from([("aws_lambda_api".into(), lambda_origin)])),
        all_tools(),
    )
    .await;
    let agent_tools = fx.chat.tools.clone().unwrap();
    assert!(
        agent_tools
            .hosts
            .register("bad", Some("not-an-arn"))
            .await
            .is_err()
    );
    let (host, token) = agent_tools
        .hosts
        .register("lambda-tools", Some(arn))
        .await
        .unwrap();
    assert!(
        token.is_none() && host.available,
        "Lambda hosts hold no token and no connection"
    );
    assert_eq!(
        host.tools[0].name, "restart",
        "registration reads the function's tools"
    );
    let source = agent_tools
        .add_source(fx.agent, Target::ToolHost(host.id), &["restart".into()])
        .await
        .unwrap();
    assert_eq!(source.slug, "lambda-tools");
    let tools = fx.tools().await;
    assert!(tools.iter().any(|t| t["name"] == "lambda-tools.restart"));
    let result = fx
        .invoke("lambda-tools.restart", json!({"service":"search"}))
        .await;
    let output: Value = serde_json::from_str(
        result["outputJson"]
            .as_str()
            .unwrap_or_else(|| panic!("{result}")),
    )
    .unwrap();
    assert_eq!(output, json!({"restarted":"search"}));

    // The same tool in the background: a ticket now, the outcome as input to the agent later.
    assert!(
        tools.iter().all(|t| t["name"] != "tools.result"),
        "offered only with background tools"
    );
    agent_tools
        .set_tool(
            source.id,
            "restart",
            &tilde::tools::ToolSettings {
                is_async: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    fx.next_invocation().await;
    let tools = fx.tools().await;
    assert_eq!(
        tools
            .iter()
            .find(|t| t["name"] == "lambda-tools.restart")
            .unwrap()["detached"],
        true
    );
    assert!(tools.iter().any(|t| t["name"] == "tools.result"));
    // Under the interrupt policy a new message cancels the running work; a background result
    // must not, since that work is what asked for it.
    let pg = fx.db.pool.get().await.unwrap();
    pg.execute(
        "UPDATE agents SET concurrency_policy='interrupt' WHERE id=$1",
        &[&fx.agent],
    )
    .await
    .unwrap();
    let started = std::time::Instant::now();
    let accepted = fx
        .invoke("lambda-tools.restart", json!({"service":"slow"}))
        .await;
    let accepted: Value = serde_json::from_str(
        accepted["outputJson"]
            .as_str()
            .unwrap_or_else(|| panic!("{accepted}")),
    )
    .unwrap();
    assert!(
        started.elapsed() < std::time::Duration::from_millis(500),
        "the ticket does not wait for the host"
    );
    assert_eq!(accepted["status"], "processing");
    let ticket = accepted["ticket"].as_str().unwrap().to_owned();
    let pending = fx.invoke("tools.result", json!({"ticket":ticket})).await;
    let pending: Value = serde_json::from_str(pending["outputJson"].as_str().unwrap()).unwrap();
    assert_eq!(pending["status"], "processing");
    let ticket_id = Uuid::parse_str(&ticket).unwrap();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
    let woken = loop {
        if let Some(row) = pg
            .query_opt("SELECT text FROM chat_inputs WHERE id=$1", &[&ticket_id])
            .await
            .unwrap()
        {
            break row.get::<_, String>(0);
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "the agent was never told"
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    };
    assert!(
        woken.contains("lambda-tools.restart")
            && woken.contains("completed")
            && woken.contains("slow"),
        "{woken}"
    );
    let status: String = pg
        .query_one(
            "SELECT i.status FROM chat_inputs n JOIN chat_invocations i ON i.id=n.invocation_id WHERE n.id=$1",
            &[&ticket_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        status, "running",
        "the result is queued, not an interruption"
    );
    let done = fx.invoke("tools.result", json!({"ticket":ticket})).await;
    let done: Value = serde_json::from_str(done["outputJson"].as_str().unwrap()).unwrap();
    assert_eq!(
        done,
        json!({"tool":"lambda-tools.restart","status":"completed","output":{"restarted":"slow"}})
    );
    // The call's span ends with its ticket; the host's work is a child span of its own.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    while spans
        .get_finished_spans()
        .unwrap()
        .iter()
        .all(|s| s.name != "background lambda-tools.restart")
    {
        assert!(tokio::time::Instant::now() < deadline, "no background span");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let work = common::span(&spans, "background lambda-tools.restart");
    let call = spans
        .get_finished_spans()
        .unwrap()
        .into_iter()
        .find(|s| s.span_context.span_id() == work.parent_span_id)
        .expect("the background span's parent is the call's span");
    assert_eq!(call.name, "execute_tool lambda-tools.restart");
    assert!(
        call.attributes
            .iter()
            .any(|a| a.key.as_str() == "tilde.tool.background"
                && a.value == opentelemetry::Value::Bool(true))
    );
    assert!(
        work.end_time >= call.end_time,
        "the work outlives the ticket"
    );

    // A Lambda host's instance: verified at setup, its key in every invoke payload.
    let (crm, _) = agent_tools
        .hosts
        .register(
            "lambda-crm",
            Some("arn:aws:lambda:eu-central-1:123456789012:function:crm"),
        )
        .await
        .unwrap();
    assert_eq!(crm.provider_id.as_deref(), Some("lambda-crm"));
    let instance = Uuid::new_v4();
    let started = fx
        .connections
        .start(instance, "Lambda CRM", "lambda-crm", "api_key", &[])
        .await
        .unwrap();
    let url = url::Url::parse(&started.brokering_url).unwrap();
    let setup = Uuid::parse_str(url.path_segments().unwrap().next_back().unwrap()).unwrap();
    let setup_token = url
        .query_pairs()
        .find(|(key, _)| key == "connection_setup_token")
        .unwrap()
        .1
        .into_owned();
    let key = |value: &str| {
        [("api_key".to_string(), secrecy::SecretString::from(value))]
            .into_iter()
            .collect::<tilde::connections::model::Values>()
    };
    let view = fx.connections.view(setup, &setup_token).await.unwrap();
    let refused = fx
        .connections
        .advance(setup, &setup_token, view.action_id, key("other-key"))
        .await
        .err()
        .unwrap();
    assert_eq!(refused.to_string(), "Unknown key");
    let view = fx.connections.view(setup, &setup_token).await.unwrap();
    fx.connections
        .advance(setup, &setup_token, view.action_id, key("lambda-key"))
        .await
        .unwrap();
    assert_eq!(
        fx.connections
            .get(instance)
            .await
            .unwrap()
            .account_label
            .as_deref(),
        Some("ops")
    );
    let crm = fx.use_tools(instance, &["restart"]).await;
    assert_eq!(crm, "lambda-crm_lambda_crm");
    fx.next_invocation().await;
    let result = fx
        .invoke(&format!("{crm}.restart"), json!({"service":"crm"}))
        .await;
    let output: Value = serde_json::from_str(
        result["outputJson"]
            .as_str()
            .unwrap_or_else(|| panic!("{result}")),
    )
    .unwrap();
    assert_eq!(output, json!({"key":"lambda-key"}));
    drop(pg);
    function.abort();
    fx.close().await;
}

/// A host that needs credentials: it publishes a provider, Tilde runs each instance's setup and
/// asks the host to verify the result, and every call on an instance carries its credentials.
#[tokio::test]
async fn provider_host_instances_are_verified_and_called_with_their_own_credentials() {
    let fx = Fixture::new(33, Endpoints::default(), all_tools()).await;
    let agent_tools = fx.chat.tools.clone().unwrap();
    let (host, token) = agent_tools.hosts.register("crm-host", None).await.unwrap();
    let bearer = token.unwrap().expose_secret().to_owned();
    let provider = json!({"id":"acme-crm","name":"Acme CRM","connectionTypes":[{"id":"api_key","name":"API key",
        "static":{"schemaJson":json!({"type":"object","properties":{"api_key":{"type":"string","title":"API key","minLength":1,"writeOnly":true}},
            "required":["api_key"],"additionalProperties":false}).to_string()}}]});
    let search = json!({"name":"search","description":"Search the CRM.",
        "inputSchemaJson":json!({"type":"object","properties":{"q":{"type":"string"}},"required":["q"]}).to_string()});
    let origin = fx.origin.clone();
    let (registered, mut ready) = tokio::sync::mpsc::channel::<()>(1);
    let host_process = tokio::spawn(async move {
        let http = reqwest::Client::new();
        let base = format!("{origin}/tilde.tool_host.v1.ToolHostService");
        let watch = http
            .post(format!("{base}/Watch"))
            .bearer_auth(&bearer)
            .header("content-type", "application/connect+json")
            .header("connect-protocol-version", "1")
            .body(framed(json!({"tools":[search],"provider":provider})))
            .send()
            .await
            .unwrap();
        assert!(watch.status().is_success());
        let mut stream = watch.bytes_stream();
        let mut buffer = vec![];
        let key = |frame: &Value| {
            frame["credentials"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| f["key"] == "api_key")
                .map(|f| f["value"].as_str().unwrap().to_owned())
                .unwrap()
        };
        while let Some(chunk) = stream.next().await {
            buffer.extend_from_slice(&chunk.unwrap());
            for frame in drain(&mut buffer) {
                if frame.get("registered").is_some() {
                    registered.send(()).await.unwrap();
                }
                let outcome = if let Some(verify) = frame.get("verify") {
                    assert_eq!(verify["connectionType"], "api_key");
                    if key(verify) == "good-key" {
                        json!({"callId":verify["callId"],"outputJson":"{}","accountLabel":"acme-eu"})
                    } else {
                        json!({"callId":verify["callId"],"error":"That API key was refused"})
                    }
                } else if let Some(call) = frame.get("call") {
                    let input: Value =
                        serde_json::from_str(call["inputJson"].as_str().unwrap()).unwrap();
                    json!({"callId":call["callId"],"outputJson":json!({
                        "key":key(call),"connection":call["connectionId"],"method":call["connectionType"],"q":input["q"]}).to_string()})
                } else {
                    continue;
                };
                let done = http
                    .post(format!("{base}/Respond"))
                    .bearer_auth(&bearer)
                    .json(&outcome)
                    .send()
                    .await
                    .unwrap();
                assert!(done.status().is_success());
            }
        }
    });
    ready.recv().await.unwrap();

    // The host's provider is registered as a tool provider that only the host may redefine.
    // The health sweep samples its live stream into the twelve-hour history.
    tilde::agent::health::AgentHealth::new(fx.db.pool.clone())
        .poll_once()
        .await
        .unwrap();
    let listed = agent_tools.hosts.get(host.id).await.unwrap();
    assert_eq!(listed.provider_id.as_deref(), Some("acme-crm"));
    assert_eq!(listed.auth_methods, ["API key"]);
    assert_eq!(listed.health_history.len(), 12);
    let latest = listed.health_history.last().unwrap();
    assert_eq!((latest.total_checks, latest.failed_checks), (1, 0));
    let registered = fx.connections.provider("acme-crm").await.unwrap();
    assert_eq!(
        registered.connection_types[0].capabilities,
        [tilde::connections::model::Capability::Tool]
    );
    assert!(
        fx.connections
            .register_provider(registered, None)
            .await
            .is_err(),
        "a client cannot take over a tool host's provider"
    );
    assert!(
        agent_tools
            .add_source(fx.agent, Target::ToolHost(host.id), &["search".into()])
            .await
            .is_err(),
        "its tools need an instance's credentials"
    );

    // An instance is set up through the setup broker; the host refuses a bad key in the form.
    let instance = Uuid::new_v4();
    let started = fx
        .connections
        .start(instance, "Acme CRM", "acme-crm", "api_key", &[])
        .await
        .unwrap();
    let url = url::Url::parse(&started.brokering_url).unwrap();
    let setup = Uuid::parse_str(url.path_segments().unwrap().next_back().unwrap()).unwrap();
    let setup_token = url
        .query_pairs()
        .find(|(key, _)| key == "connection_setup_token")
        .unwrap()
        .1
        .into_owned();
    let key = |value: &str| {
        [("api_key".to_string(), secrecy::SecretString::from(value))]
            .into_iter()
            .collect::<tilde::connections::model::Values>()
    };
    let view = fx.connections.view(setup, &setup_token).await.unwrap();
    let refused = fx
        .connections
        .advance(setup, &setup_token, view.action_id, key("stolen-key"))
        .await
        .err()
        .expect("the host refuses the key");
    assert_eq!(refused.to_string(), "That API key was refused");
    let view = fx.connections.view(setup, &setup_token).await.unwrap();
    assert_eq!(view.step, "fields", "back on the form, not failed");
    assert_eq!(
        fx.connections.get(instance).await.unwrap().status,
        "requires_action"
    );
    fx.connections
        .advance(setup, &setup_token, view.action_id, key("good-key"))
        .await
        .unwrap();
    let ready = fx.connections.get(instance).await.unwrap();
    assert_eq!(ready.status, "ready");
    assert_eq!(ready.account_label.as_deref(), Some("acme-eu"));

    // The agent's tools on it call the host with this instance's credentials.
    let crm = fx.use_tools(instance, &["search"]).await;
    let result = fx
        .invoke(&format!("{crm}.search"), json!({"q":"renewals"}))
        .await;
    let output: Value = serde_json::from_str(
        result["outputJson"]
            .as_str()
            .unwrap_or_else(|| panic!("{result}")),
    )
    .unwrap();
    assert_eq!(
        output,
        json!({"key":"good-key","connection":instance.to_string(),"method":"api_key","q":"renewals"})
    );
    let pg = fx.db.pool.get().await.unwrap();
    let queued: i64 = pg
        .query_one("SELECT COUNT(*) FROM tool_host_calls", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(queued, 0, "sealed credentials leave with their queue rows");

    // Deleting the host takes its provider and instances with it.
    agent_tools.hosts.delete(host.id).await.unwrap();
    assert!(fx.connections.get(instance).await.is_err());
    assert!(fx.connections.provider("acme-crm").await.is_err());
    assert!(
        fx.tools()
            .await
            .iter()
            .all(|t| t["name"] != "crm_eu.search")
    );
    drop(pg);
    host_process.abort();
    fx.close().await;
}

/// Dial Watch and return the first frame: `registered` once published, or the stream's error.
async fn watch(origin: &str, bearer: &str, body: Value) -> (Value, reqwest::Response) {
    let response = reqwest::Client::new()
        .post(format!("{origin}/tilde.tool_host.v1.ToolHostService/Watch"))
        .bearer_auth(bearer)
        .header("content-type", "application/connect+json")
        .header("connect-protocol-version", "1")
        .body(framed(body))
        .send()
        .await
        .unwrap();
    let mut response = response;
    let mut buffer = vec![];
    loop {
        if let Some(frame) = drain(&mut buffer).into_iter().next() {
            return (frame, response);
        }
        match response.chunk().await.unwrap() {
            Some(chunk) => buffer.extend_from_slice(&chunk),
            None => return (json!({"error":"stream ended"}), response),
        }
    }
}

/// A host publishes its provider's OAuth endpoints itself, so it cannot aim Tilde's token request
/// at a private address.
#[tokio::test]
async fn host_published_oauth_cannot_reach_private_endpoints() {
    let fx = Fixture::new(34, Endpoints::default(), all_tools()).await;
    let hosts = fx.chat.tools.clone().unwrap().hosts;
    // A loopback "token endpoint" that only counts who reaches it.
    let token_endpoint = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = token_endpoint.local_addr().unwrap().port();
    let reached = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = reached.clone();
    let listener = tokio::spawn(async move {
        while token_endpoint.accept().await.is_ok() {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    });
    let provider = |id: &str, grant: &str, client: &str, token_url: String| {
        json!({"id":id,"name":"Attacker","connectionTypes":[{"id":"oauth","name":"Sign in",
            "oauth":{"grant":grant,"configuration":{"tokenUrl":token_url,
                "authorizationUrl":"https://accounts.example.com/authorize","client":client}}}]})
    };

    let (_, token) = hosts.register("attacker", None).await.unwrap();
    let bearer = token.unwrap().expose_secret().to_owned();
    // A host's endpoints must be HTTPS, and its token endpoint a public address when called.
    let (frame, _) = watch(
        &fx.origin,
        &bearer,
        json!({"tools":[echo_tool()],"provider":provider("attacker-crm","O_AUTH_GRANT_CLIENT_CREDENTIALS",
            "O_AUTH_CLIENT_FORM",format!("http://127.0.0.1:{port}/token"))}),
    )
    .await;
    assert!(frame.get("error").is_some(), "{frame}");
    let (frame, _stream) = watch(
        &fx.origin,
        &bearer,
        json!({"tools":[echo_tool()],"provider":provider("attacker-crm","O_AUTH_GRANT_CLIENT_CREDENTIALS",
            "O_AUTH_CLIENT_FORM",format!("https://127.0.0.1:{port}/token"))}),
    )
    .await;
    assert!(frame.get("registered").is_some(), "{frame}");
    let started = fx
        .connections
        .start(Uuid::new_v4(), "CRM", "attacker-crm", "oauth", &[])
        .await
        .unwrap();
    let url = url::Url::parse(&started.brokering_url).unwrap();
    let setup = Uuid::parse_str(url.path_segments().unwrap().next_back().unwrap()).unwrap();
    let setup_token = url
        .query_pairs()
        .find(|(key, _)| key == "connection_setup_token")
        .unwrap()
        .1
        .into_owned();
    let view = fx.connections.view(setup, &setup_token).await.unwrap();
    let values: tilde::connections::model::Values = [
        ("client_id".to_string(), SecretString::from("client")),
        ("client_secret".to_string(), SecretString::from("secret")),
    ]
    .into_iter()
    .collect();
    assert!(
        fx.connections
            .advance(setup, &setup_token, view.action_id, values)
            .await
            .is_err()
    );
    assert_eq!(
        reached.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "nothing reached the loopback token endpoint"
    );
    listener.abort();
    fx.close().await;
}
