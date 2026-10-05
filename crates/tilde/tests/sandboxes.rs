//! Sandboxes across their boundaries: a blueprint and an agent's sandbox setting in Postgres, the
//! agent's sandbox tools called from its invocation, the VM launched, paused, resumed and
//! terminated through a fake E2B, the in-VM process enrolling and serving operations over its own
//! stream, and that process calling the blueprint's tools on a connected tool host.
mod common;
use common::invocation::{Fixture, all_tools, drain, framed};
use futures::StreamExt;
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tilde::{
    connections::catalog::Endpoints,
    encryption::{Encryption, SecretBinding},
    sandboxes::Timings,
    tools::{Owner, Target},
};
use uuid::Uuid;

/// What the fake provider and the fake VM processes saw and hold.
#[derive(Default)]
struct World {
    /// Requests to the E2B API, as `METHOD path`.
    e2b: Vec<String>,
    templates: Vec<String>,
    /// Environment each started process received.
    started: Vec<BTreeMap<String, String>>,
    /// Files of the VM.
    files: BTreeMap<String, String>,
    /// The running process's session token and its task, per VM.
    sessions: BTreeMap<String, String>,
    processes: BTreeMap<String, tokio::task::JoinHandle<()>>,
    paused: Vec<String>,
    /// Fail the next connect, as an E2B outage would.
    fail_connect: bool,
}
type Shared = Arc<Mutex<World>>;

/// The `tilde sandbox connect` process: connect with `token`, keep a session token issued for
/// it, and answer operations. `call-tool` calls the blueprint's credential tool through Tilde, as
/// a command run by the operation (which passes its ID on).
async fn process(origin: String, vm: String, token: String, world: Shared) {
    let http = reqwest::Client::new();
    let base = format!("{origin}/tilde.sandbox.v1.SandboxService");
    let connected = http
        .post(format!("{base}/Connect"))
        .bearer_auth(&token)
        .header("content-type", "application/connect+json")
        .header("connect-protocol-version", "1")
        .body(framed(json!({})))
        .send()
        .await
        .unwrap();
    let mut stream = connected.bytes_stream();
    let mut buffer = vec![];
    let mut session = token;
    let mut env = BTreeMap::new();
    while let Some(chunk) = stream.next().await {
        buffer.extend_from_slice(&chunk.unwrap());
        for frame in drain(&mut buffer) {
            if let Some(registered) = frame.get("registered") {
                if let Some(issued) = registered["sessionToken"].as_str() {
                    session = issued.to_owned();
                }
                world
                    .lock()
                    .unwrap()
                    .sessions
                    .insert(vm.clone(), session.clone());
                for var in registered["env"].as_array().into_iter().flatten() {
                    env.insert(
                        var["name"].as_str().unwrap().to_owned(),
                        var["value"].clone(),
                    );
                }
            }
            let Some(op) = frame.get("operation") else {
                continue;
            };
            let input: Value = serde_json::from_str(op["inputJson"].as_str().unwrap()).unwrap();
            let path = input["path"].as_str().unwrap_or_default().to_owned();
            let output = match (op["name"].as_str().unwrap(), input["command"].as_str()) {
                ("exec", Some("printenv SECRET")) => {
                    json!({"exit_code":0,"stdout":env["SECRET"],"stderr":"","timed_out":false,"truncated":false})
                }
                ("exec", Some("call-tool")) => {
                    let answer: Value = http
                        .post(format!("{base}/InvokeTool"))
                        .bearer_auth(&session)
                        .json(&json!({"name":"creds.git_credentials","operationId":op["id"],"inputJson":json!({"host":"github.com","path":"acme/api"}).to_string()}))
                        .send()
                        .await
                        .unwrap()
                        .json()
                        .await
                        .unwrap();
                    json!({"exit_code":0,"stdout":answer["outputJson"],"stderr":"","timed_out":false,"truncated":false})
                }
                ("write_file", _) => {
                    let content = input["content"].as_str().unwrap().to_owned();
                    world.lock().unwrap().files.insert(path, content.clone());
                    json!({"bytes_written":content.len()})
                }
                ("read_file", _) => {
                    let content = world.lock().unwrap().files[&path].clone();
                    json!({"content":content,"total_lines":content.lines().count(),"truncated":false})
                }
                other => panic!("unexpected operation {other:?}"),
            };
            let done = http
                .post(format!("{base}/Respond"))
                .bearer_auth(&session)
                .json(&json!({"id":op["id"],"outputJson":output.to_string()}))
                .send()
                .await
                .unwrap();
            assert!(done.status().is_success(), "{}", done.text().await.unwrap());
        }
    }
}

fn frame(flags: u8, value: Value) -> Vec<u8> {
    let mut out = framed(value);
    out[0] = flags;
    out
}

/// E2B's API and envd. Starting a process spawns `process`; pausing a VM freezes its process and
/// resuming it reconnects with the session it had, as a paused VM's process does.
async fn e2b(world: Shared, gateway: Arc<Mutex<String>>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let app = axum::Router::new().fallback(
        move |method: axum::http::Method, uri: axum::http::Uri, body: axum::body::Bytes| {
            let world = world.clone();
            let gateway = gateway.lock().unwrap().clone();
            async move {
                let (status, body) = answer(world, gateway, method, uri, body);
                (axum::http::StatusCode::from_u16(status).unwrap(), body)
            }
        },
    );
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    origin
}
fn answer(
    world: Shared,
    gateway: String,
    method: axum::http::Method,
    uri: axum::http::Uri,
    body: axum::body::Bytes,
) -> (u16, Vec<u8>) {
    {
        let path = uri.path().to_owned();
        let ok = |value: Value| (200, serde_json::to_vec(&value).unwrap());
        if path == "/envd/process.Process/Start" {
            let request: Value = serde_json::from_slice(&body[5..]).unwrap();
            let envs: BTreeMap<String, String> =
                serde_json::from_value(request["process"]["envs"].clone()).unwrap();
            let token = envs["TILDE_SANDBOX_TOKEN"].clone();
            let mut w = world.lock().unwrap();
            w.started.push(envs);
            let task = tokio::spawn(process(gateway, "sbx-1".into(), token, world.clone()));
            w.processes.insert("sbx-1".into(), task);
            let mut stream = frame(0, json!({"event":{"start":{"pid":7}}}));
            stream.extend(frame(
                0,
                json!({"event":{"end":{"exitCode":0,"status":"exited"}}}),
            ));
            stream.extend(frame(2, json!({})));
            return (200, stream);
        }
        let mut w = world.lock().unwrap();
        w.e2b.push(format!("{method} {path}"));
        match (method.as_str(), path.as_str()) {
            ("POST", "/e2b/sandboxes") => {
                let request: Value = serde_json::from_slice(&body).unwrap();
                assert_eq!(request["autoPause"], true);
                w.templates
                    .push(request["templateID"].as_str().unwrap().to_owned());
                ok(json!({"sandboxID":"sbx-1"}))
            }
            ("POST", "/e2b/sandboxes/sbx-1/connect") if w.fail_connect => {
                w.fail_connect = false;
                (503, b"{}".to_vec())
            }
            ("POST", "/e2b/sandboxes/sbx-1/connect") => {
                if w.paused.pop().is_some() {
                    let session = w.sessions["sbx-1"].clone();
                    let task =
                        tokio::spawn(process(gateway, "sbx-1".into(), session, world.clone()));
                    w.processes.insert("sbx-1".into(), task);
                }
                ok(json!({"sandboxID":"sbx-1","envdAccessToken":"envd","sandboxDomain":"e2b.test"}))
            }
            ("POST", "/e2b/sandboxes/sbx-1/pause") => {
                w.paused.push("sbx-1".into());
                if let Some(task) = w.processes.remove("sbx-1") {
                    task.abort();
                }
                ok(json!({}))
            }
            ("DELETE", "/e2b/sandboxes/sbx-1") => {
                if let Some(task) = w.processes.remove("sbx-1") {
                    task.abort();
                }
                (204, vec![])
            }
            ("POST", "/envd/filesystem.Filesystem/MakeDir") => ok(json!({})),
            other => panic!("unexpected E2B request {other:?}"),
        }
    }
}

/// A connected tool host vending narrowed git credentials; it records who asked.
async fn credential_host(fx: &Fixture, asked: Arc<Mutex<Vec<Value>>>) -> Uuid {
    let tools = fx.chat.tools.clone().unwrap();
    let (host, token) = tools.hosts.register("creds", None).await.unwrap();
    let bearer = token.unwrap().expose_secret().to_owned();
    let base = format!("{}/tilde.tool_host.v1.ToolHostService", fx.origin);
    let (ready, mut registered) = tokio::sync::mpsc::channel::<()>(1);
    tokio::spawn(async move {
        let http = reqwest::Client::new();
        let tool = json!({"name":"git_credentials","description":"Credentials for one repository.",
            "inputSchemaJson":json!({"type":"object","properties":{"host":{"type":"string"},"path":{"type":"string"}},"required":["host","path"]}).to_string()});
        let watch = http
            .post(format!("{base}/Watch"))
            .bearer_auth(&bearer)
            .header("content-type", "application/connect+json")
            .header("connect-protocol-version", "1")
            .body(framed(json!({"tools":[tool]})))
            .send()
            .await
            .unwrap();
        let mut stream = watch.bytes_stream();
        let mut buffer = vec![];
        while let Some(chunk) = stream.next().await {
            buffer.extend_from_slice(&chunk.unwrap());
            for frame in drain(&mut buffer) {
                if frame.get("registered").is_some() {
                    ready.send(()).await.unwrap();
                }
                let Some(call) = frame.get("call") else {
                    continue;
                };
                asked.lock().unwrap().push(call.clone());
                http.post(format!("{base}/Respond"))
                    .bearer_auth(&bearer)
                    .json(&json!({"callId":call["callId"],"outputJson":json!({"username":"x-access-token","password":"ghs_narrow"}).to_string()}))
                    .send()
                    .await
                    .unwrap();
            }
        }
    });
    registered.recv().await.unwrap();
    host.id
}

async fn e2b_connection(fx: &Fixture) -> Uuid {
    let id = Uuid::new_v4();
    fx.connections
        .start(id, "e2b", "e2b", "api", &[])
        .await
        .unwrap();
    let crypto = Encryption::initialize(&fx.db.pool, common::seed(53))
        .await
        .unwrap();
    let sealed = crypto
        .seal(
            SecretBinding {
                resource_kind: "connection",
                resource_id: id,
                name: "api_key",
            },
            &SecretString::from("e2b_key"),
        )
        .unwrap()
        .into_bytes();
    let pg = fx.db.pool.get().await.unwrap();
    pg.execute(
        "INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES($1,'api_key',$2)",
        &[&id, &sealed],
    )
    .await
    .unwrap();
    pg.execute("UPDATE connections SET status='ready' WHERE id=$1", &[&id])
        .await
        .unwrap();
    id
}

#[tokio::test]
async fn agent_sandbox_is_launched_served_slept_woken_and_terminated() {
    let spans = common::capture_spans();
    let world: Shared = Default::default();
    // The fake learns the gateway's address once the fixture listens.
    let gateway: Arc<Mutex<String>> = Default::default();
    let provider = e2b(world.clone(), gateway.clone()).await;
    let endpoints = Endpoints(BTreeMap::from([
        ("e2b_api".to_owned(), format!("{provider}/e2b")),
        ("e2b_envd".to_owned(), format!("{provider}/envd")),
    ]));
    let mut fx = Fixture::new(53, endpoints, all_tools()).await;
    *gateway.lock().unwrap() = fx.origin.clone();
    let tools = fx.chat.tools.clone().unwrap();
    let sandboxes = tools.sandboxes.clone();

    // A blueprint with an encrypted variable and a credential tool for processes inside. Its
    // sandboxes sleep after an hour unused.
    let connection = e2b_connection(&fx).await;
    let hour = Timings {
        sleep_after: 3600,
        terminate_after: 0,
        connect_timeout: 0,
    };
    let blueprint = sandboxes
        .create_blueprint("builder", connection, "tilde-base", "thread", hour)
        .await
        .unwrap();
    assert_eq!(
        blueprint.timings.terminate_after,
        7 * 86_400,
        "unset timings take defaults"
    );
    sandboxes
        .set_env(blueprint.id, "SECRET", &SecretString::from("s3cret"))
        .await
        .unwrap();
    let asked: Arc<Mutex<Vec<Value>>> = Default::default();
    let host = credential_host(&fx, asked.clone()).await;
    tools
        .add_source(
            Owner::Blueprint(blueprint.id),
            Target::ToolHost(host),
            &["git_credentials".into()],
        )
        .await
        .unwrap();
    let stored: Vec<u8> = fx
        .db
        .pool
        .get()
        .await
        .unwrap()
        .query_one("SELECT encrypted_value FROM sandbox_blueprint_env", &[])
        .await
        .unwrap()
        .get(0);
    assert!(!String::from_utf8_lossy(&stored).contains("s3cret"));

    // The agent's sandbox setting gives it the sandbox tools; its first call launches the VM.
    sandboxes
        .set_agent_sandbox(fx.agent, blueprint.id)
        .await
        .unwrap();
    fx.next_invocation().await;
    let names: Vec<Value> = fx
        .tools()
        .await
        .into_iter()
        .map(|t| t["name"].clone())
        .collect();
    for tool in ["sandbox.exec", "sandbox.apply_patch", "sandbox.grep"] {
        assert!(names.contains(&json!(tool)), "{tool} in {names:?}");
    }
    let echoed = fx
        .output("sandbox.exec", json!({"command":"printenv SECRET"}))
        .await;
    assert_eq!(
        echoed["stdout"], "s3cret",
        "the blueprint's environment reaches the VM"
    );
    let (enrollment, started) = {
        let w = world.lock().unwrap();
        assert_eq!(w.templates, ["tilde-base"]);
        assert_eq!(w.started.len(), 1);
        (
            w.started[0]["TILDE_SANDBOX_TOKEN"].clone(),
            w.started[0]["TILDE_URL"].clone(),
        )
    };
    assert_eq!(
        started, "http://127.0.0.1:1",
        "the process dials the runtime URL"
    );
    let replayed = reqwest::Client::new()
        .post(format!(
            "{}/tilde.sandbox.v1.SandboxService/ListTools",
            fx.origin
        ))
        .bearer_auth(&enrollment)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(replayed.status(), 401, "the enrollment token is single-use");

    // A process in the VM calls the blueprint's tool; its output reaches the process only.
    let called = fx
        .output("sandbox.exec", json!({"command":"call-tool"}))
        .await;
    let vended: Value = serde_json::from_str(called["stdout"].as_str().unwrap()).unwrap();
    assert_eq!(vended["password"], "ghs_narrow");
    let sandbox = sandboxes
        .list(Some(blueprint.id), None)
        .await
        .unwrap()
        .remove(0);
    {
        let asked = asked.lock().unwrap();
        assert_eq!(asked[0]["sandboxId"], sandbox.id.to_string());
        assert_eq!(asked[0]["agentId"], fx.agent.to_string());
        assert_eq!(asked[0]["threadId"], fx.thread.to_string());
    }
    // It is traced in the agent's invocation, inside the sandbox.exec call that ran the command.
    let inner = common::span(&spans, "execute_tool creds.git_credentials");
    let attribute = |span: &opentelemetry_sdk::trace::SpanData, key: &str| {
        span.attributes
            .iter()
            .find(|a| a.key.as_str() == key)
            .map(|a| a.value.to_string())
    };
    let exec = spans
        .get_finished_spans()
        .unwrap()
        .into_iter()
        .find(|s| s.span_context.span_id() == inner.parent_span_id)
        .expect("the call's parent span");
    assert_eq!(exec.name, "execute_tool sandbox.exec");
    assert_eq!(
        attribute(&inner, "tilde.agent.id"),
        Some(fx.agent.to_string())
    );
    assert_eq!(
        attribute(&inner, "tilde.invocation.id"),
        attribute(&exec, "tilde.invocation.id")
    );
    assert!(
        attribute(&inner, "tilde.observation.input")
            .unwrap()
            .contains("acme/api")
    );
    // A call outside any operation (a background job's) belongs to the latest invocation.
    let session = world.lock().unwrap().sessions["sbx-1"].clone();
    let detached = reqwest::Client::new()
        .post(format!("{}/tilde.sandbox.v1.SandboxService/InvokeTool", fx.origin))
        .bearer_auth(&session)
        .json(&json!({"name":"creds.git_credentials","inputJson":json!({"host":"github.com","path":"acme/web"}).to_string()}))
        .send()
        .await
        .unwrap();
    assert!(detached.status().is_success());
    let background = spans
        .get_finished_spans()
        .unwrap()
        .into_iter()
        .find(|s| attribute(s, "tilde.observation.input").is_some_and(|i| i.contains("acme/web")))
        .expect("the background call's span");
    assert_eq!(
        attribute(&background, "tilde.invocation.id"),
        attribute(&exec, "tilde.invocation.id")
    );
    assert_eq!(
        background.parent_span_id,
        opentelemetry::trace::SpanId::INVALID
    );

    // apply_patch is read and written through the process's file operations.
    fx.output(
        "sandbox.write_file",
        json!({"path":"notes.txt","content":"one\ntwo\n"}),
    )
    .await;
    let patched = fx
        .output(
            "sandbox.apply_patch",
            json!({"patch":"*** Begin Patch\n*** Update File: notes.txt\n@@\n one\n-two\n+three\n*** End Patch"}),
        )
        .await;
    assert_eq!(patched["modified"], json!(["notes.txt"]));
    assert_eq!(world.lock().unwrap().files["notes.txt"], "one\nthree\n");

    // Idle past its blueprint's sleep_after, it is paused; the next call resumes the VM, whose
    // process reconnects by itself.
    let pg = fx.db.pool.get().await.unwrap();
    pg.execute(
        "UPDATE sandboxes SET last_used_at=NOW()-INTERVAL '20 minutes'",
        &[],
    )
    .await
    .unwrap();
    sandboxes.sweep().await.unwrap();
    let awake = sandboxes.list(Some(blueprint.id), None).await.unwrap();
    assert_eq!(awake[0].status, "running", "20 minutes is within the hour");
    // A VM E2B would soon pause by itself has its TTL renewed.
    pg.execute(
        "UPDATE sandboxes SET expires_at=NOW()+INTERVAL '5 minutes'",
        &[],
    )
    .await
    .unwrap();
    let connects = |w: &World| {
        w.e2b
            .iter()
            .filter(|r| *r == "POST /e2b/sandboxes/sbx-1/connect")
            .count()
    };
    let before = connects(&world.lock().unwrap());
    // Never while another process holds the sandbox, mid-transition.
    pg.execute(
        "UPDATE sandboxes SET lease_until=NOW()+INTERVAL '1 hour'",
        &[],
    )
    .await
    .unwrap();
    sandboxes.sweep().await.unwrap();
    assert_eq!(connects(&world.lock().unwrap()), before);
    pg.execute("UPDATE sandboxes SET lease_until=NULL", &[])
        .await
        .unwrap();
    // A failed renewal gives the lease back, so agents are not kept waiting on it.
    world.lock().unwrap().fail_connect = true;
    sandboxes.sweep().await.unwrap();
    let leased: bool = pg
        .query_one("SELECT lease_until IS NOT NULL FROM sandboxes", &[])
        .await
        .unwrap()
        .get(0);
    assert!(!leased, "the lease is released after a failed renewal");
    sandboxes.sweep().await.unwrap();
    assert_eq!(connects(&world.lock().unwrap()), before + 2);
    let renewed: bool = pg
        .query_one(
            "SELECT expires_at > NOW()+INTERVAL '50 minutes' FROM sandboxes",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert!(renewed, "the new deadline is recorded");
    // Past a shorter sleep_after it sleeps, but not while an operation is still in flight.
    pg.execute(
        "INSERT INTO sandbox_calls(id,sandbox_id,operation,input_json,invocation_id) \
         SELECT gen_random_uuid(),id,'exec','{}',gen_random_uuid() FROM sandboxes",
        &[],
    )
    .await
    .unwrap();
    let ten_minutes = Timings {
        sleep_after: 600,
        terminate_after: 0,
        connect_timeout: 0,
    };
    sandboxes
        .update_blueprint(blueprint.id, None, None, None, None, Some(ten_minutes))
        .await
        .unwrap();
    sandboxes.sweep().await.unwrap();
    let busy = sandboxes.list(Some(blueprint.id), None).await.unwrap();
    assert_eq!(
        busy[0].status, "running",
        "an operation in flight keeps it awake"
    );
    pg.execute("DELETE FROM sandbox_calls", &[]).await.unwrap();
    // The change started a sweep of its own; whichever sweep runs puts the VM to sleep.
    for _ in 0..50 {
        sandboxes.sweep().await.unwrap();
        if sandboxes.list(Some(blueprint.id), None).await.unwrap()[0].status == "sleeping" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let slept = sandboxes.list(Some(blueprint.id), None).await.unwrap();
    assert_eq!(slept[0].status, "sleeping");
    let woken = fx
        .output("sandbox.exec", json!({"command":"printenv SECRET"}))
        .await;
    assert_eq!(woken["stdout"], "s3cret");
    {
        let w = world.lock().unwrap();
        assert!(
            w.e2b
                .contains(&"POST /e2b/sandboxes/sbx-1/pause".to_owned())
        );
        assert_eq!(
            w.started.len(),
            1,
            "the paused process resumed instead of a new one"
        );
        assert_eq!(w.templates.len(), 1, "no second VM was launched");
    }

    // Without the sandbox setting the agent loses its sandbox tools and the VM is terminated.
    // Removing the setting starts a sweep of its own; whichever sweep runs terminates the VM.
    sandboxes.remove_agent_sandbox(fx.agent).await.unwrap();
    for _ in 0..50 {
        sandboxes.sweep().await.unwrap();
        if sandboxes
            .list(Some(blueprint.id), None)
            .await
            .unwrap()
            .is_empty()
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(
        sandboxes
            .list(Some(blueprint.id), None)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        world
            .lock()
            .unwrap()
            .e2b
            .contains(&"DELETE /e2b/sandboxes/sbx-1".to_owned())
    );
    fx.next_invocation().await;
    assert!(
        fx.tools()
            .await
            .iter()
            .all(|t| !t["name"].as_str().unwrap().starts_with("sandbox."))
    );
    drop(pg);
    fx.close().await;
}
