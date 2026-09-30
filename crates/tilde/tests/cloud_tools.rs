//! AWS and Modal managed tools across their boundaries: encrypted connection credentials, a
//! tool the agent uses, the agent's catalog, and the upstream protocol each provider speaks,
//! against in-process fakes. AWS: an MCP session whose every request carries a SigV4
//! signature the fake recomputes, with the default region in `_meta`. Modal: gRPC over h2c to
//! the ModalClient service and the sandbox's task command router.
mod common;
use axum::response::IntoResponse;
use base64::Engine as _;
use common::invocation::{Fixture, all_tools};
use prost::Message as _;
use secrecy::SecretString;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tilde::{
    connections::catalog::Endpoints,
    encryption::{Encryption, SecretBinding},
};
use uuid::Uuid;

async fn connection(fx: &Fixture, provider: &str, typ: &str, values: &[(&str, &str)]) -> Uuid {
    let pg = fx.db.pool.get().await.unwrap();
    let id = Uuid::new_v4();
    pg.execute(
        "INSERT INTO connections(id,name,provider_id,type_id,status) VALUES($1,$2,$2,$3,'ready')",
        &[&id, &provider, &typ],
    )
    .await
    .unwrap();
    let crypto = Encryption::initialize(&fx.db.pool, common::seed(72))
        .await
        .unwrap();
    for (key, secret) in values {
        let sealed = crypto
            .seal(
                SecretBinding {
                    resource_kind: "connection",
                    resource_id: id,
                    name: key,
                },
                &SecretString::from(secret.to_string()),
            )
            .unwrap()
            .into_bytes();
        pg.execute("INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES($1,$2,$3)", &[&id, key, &sealed]).await.unwrap();
    }
    id
}

/// Recompute the SigV4 signature over what arrived and compare it with the one sent.
fn verify_sigv4(headers: &axum::http::HeaderMap, path: &str, body: &[u8]) {
    let authorization = headers["authorization"].to_str().unwrap();
    let date = headers["x-amz-date"].to_str().unwrap();
    assert!(
        authorization.starts_with(&format!(
            "AWS4-HMAC-SHA256 Credential=AKIDFIXTURE/{}/us-east-1/aws-mcp/aws4_request, SignedHeaders=",
            &date[..8]
        )),
        "{authorization}"
    );
    assert_eq!(headers["x-amz-security-token"], "session-fixture");
    let signed = authorization
        .split("SignedHeaders=")
        .nth(1)
        .unwrap()
        .split(',')
        .next()
        .unwrap();
    let host = headers["host"].to_str().unwrap();
    let mut request = reqwest::Client::new()
        .post(format!("http://{host}{path}"))
        .body(body.to_vec())
        .build()
        .unwrap();
    for name in signed.split(';').filter(|name| *name != "host") {
        request.headers_mut().insert(
            reqwest::header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            headers[name].as_bytes().try_into().unwrap(),
        );
    }
    let time = chrono::NaiveDateTime::parse_from_str(date, "%Y%m%dT%H%M%SZ")
        .unwrap()
        .and_utc();
    client_aws_sigv4::Signer::new(
        client_aws_sigv4::Credentials {
            access_key_id: "AKIDFIXTURE".into(),
            secret_access_key: "secret-fixture".into(),
            session_token: Some("session-fixture".into()),
        },
        "us-east-1",
        "aws-mcp",
    )
    .sign_request_at(&mut request, body, time.into())
    .unwrap();
    assert_eq!(
        request.headers()["authorization"],
        authorization,
        "signature matches"
    );
}

async fn aws_upstream(calls: Arc<Mutex<Vec<Value>>>) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let app = axum::Router::new().route(
            "/mcp",
            axum::routing::post(move |headers: axum::http::HeaderMap, body: axum::body::Bytes| {
                let calls = calls.clone();
                async move {
                    verify_sigv4(&headers, "/mcp", &body);
                    let body: Value = serde_json::from_slice(&body).unwrap();
                    let reply = |result: Value| axum::Json(json!({"jsonrpc":"2.0","id":body["id"],"result":result}));
                    match body["method"].as_str().unwrap() {
                        "initialize" => ([("mcp-session-id", "aws-1")], reply(json!({"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"aws","version":"1"}}))).into_response(),
                        "notifications/initialized" => axum::http::StatusCode::ACCEPTED.into_response(),
                        "tools/call" => {
                            assert_eq!(headers["mcp-session-id"], "aws-1");
                            calls.lock().unwrap().push(body["params"].clone());
                            reply(json!({"content":[{"type":"text","text":"{\"regions\":[\"eu-west-1\"]}"}]})).into_response()
                        }
                        _ => axum::http::StatusCode::BAD_REQUEST.into_response(),
                    }
                }
            }),
        );
        axum::serve(listener, app).await.unwrap()
    });
    (origin, server)
}

// The slice of Modal's messages the fake reads or answers with.
#[derive(Clone, PartialEq, prost::Message)]
struct Token {
    #[prost(string, tag = "1")]
    token: String,
}
#[derive(Clone, PartialEq, prost::Message)]
struct Id {
    #[prost(string, tag = "1")]
    id: String,
}
#[derive(Clone, PartialEq, prost::Message)]
struct ImageResult {
    #[prost(int32, tag = "1")]
    status: i32,
}
#[derive(Clone, PartialEq, prost::Message)]
struct ImageJoin {
    #[prost(message, optional, tag = "1")]
    result: Option<ImageResult>,
    #[prost(string, tag = "3")]
    entry_id: String,
}
#[derive(Clone, PartialEq, prost::Message)]
struct SandboxDefinition {
    #[prost(string, tag = "3")]
    image_id: String,
    #[prost(uint32, tag = "7")]
    timeout_secs: u32,
}
#[derive(Clone, PartialEq, prost::Message)]
struct SandboxCreate {
    #[prost(string, tag = "1")]
    app_id: String,
    #[prost(message, optional, tag = "2")]
    definition: Option<SandboxDefinition>,
}
#[derive(Clone, PartialEq, prost::Message)]
struct Created {
    #[prost(string, tag = "1")]
    sandbox_id: String,
    #[prost(string, tag = "3")]
    task_id: String,
}
#[derive(Clone, PartialEq, prost::Message)]
struct RouterAccess {
    #[prost(string, tag = "1")]
    jwt: String,
    #[prost(string, tag = "2")]
    url: String,
}
#[derive(Clone, PartialEq, prost::Message)]
struct ExecStart {
    #[prost(string, tag = "1")]
    task_id: String,
    #[prost(string, tag = "2")]
    exec_id: String,
    #[prost(string, repeated, tag = "3")]
    command_args: Vec<String>,
}
#[derive(Clone, PartialEq, prost::Message)]
struct StdioRead {
    #[prost(string, tag = "2")]
    exec_id: String,
    #[prost(int32, tag = "4")]
    file_descriptor: i32,
}
#[derive(Clone, PartialEq, prost::Message)]
struct Output {
    #[prost(bytes = "vec", tag = "1")]
    data: Vec<u8>,
}
#[derive(Clone, PartialEq, prost::Message)]
struct Exit {
    #[prost(int32, optional, tag = "1")]
    code: Option<i32>,
}

fn grpc(messages: Vec<Vec<u8>>) -> axum::response::Response {
    let mut body = vec![];
    for message in messages {
        body.push(0);
        body.extend_from_slice(&(message.len() as u32).to_be_bytes());
        body.extend(message);
    }
    (
        [("content-type", "application/grpc"), ("grpc-status", "0")],
        body,
    )
        .into_response()
}

/// ModalClient and the task command router on one h2c listener. Files live in `files`; the
/// fake understands the shell commands the provider sends for reads and writes.
async fn modal_upstream(
    files: Arc<Mutex<BTreeMap<String, String>>>,
    commands: Arc<Mutex<Vec<Vec<String>>>>,
) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let router_url = origin.clone();
    let server = tokio::spawn(async move {
        let app = axum::Router::new().fallback(
            move |version: axum::http::Version,
                  uri: axum::http::Uri,
                  headers: axum::http::HeaderMap,
                  body: axum::body::Bytes| {
                let (files, commands, router_url) =
                    (files.clone(), commands.clone(), router_url.clone());
                async move {
                    assert_eq!(version, axum::http::Version::HTTP_2);
                    assert_eq!(headers["content-type"], "application/grpc");
                    let request = &body[5..];
                    let (service, method) = uri.path()[1..].split_once('/').unwrap();
                    if service == "modal.task_command_router.TaskCommandRouter" {
                        assert_eq!(headers["authorization"], "Bearer jwt-1");
                        return match method {
                            "TaskExecStart" => {
                                let start = ExecStart::decode(request).unwrap();
                                assert_eq!(start.task_id, "ta-1");
                                assert_eq!(start.command_args[..2], ["bash", "-lc"]);
                                commands
                                    .lock()
                                    .unwrap()
                                    .push(vec![start.exec_id, start.command_args[2].clone()]);
                                grpc(vec![vec![]])
                            }
                            "TaskExecStdioRead" => {
                                let read = StdioRead::decode(request).unwrap();
                                let command = commands
                                    .lock()
                                    .unwrap()
                                    .iter()
                                    .find(|c| c[0] == read.exec_id)
                                    .unwrap()[1]
                                    .clone();
                                let stdout =
                                    if let Some(path) = command.strip_prefix("base64 -w0 -- ") {
                                        let file =
                                            files.lock().unwrap()[path.trim_matches('\'')].clone();
                                        base64::engine::general_purpose::STANDARD.encode(file)
                                    } else if let Some((target, data)) =
                                        command.split_once(" <<'__TILDE_EOF__'\n")
                                    {
                                        let path = target
                                            .rsplit("base64 -d > ")
                                            .next()
                                            .unwrap()
                                            .trim_matches('\'');
                                        let data = base64::engine::general_purpose::STANDARD
                                            .decode(data.trim_end_matches("\n__TILDE_EOF__"))
                                            .unwrap();
                                        files
                                            .lock()
                                            .unwrap()
                                            .insert(path.into(), String::from_utf8(data).unwrap());
                                        String::new()
                                    } else {
                                        format!("ran: {command}\n")
                                    };
                                // Output arrives in pieces; stderr is empty.
                                grpc(if read.file_descriptor == 0 && !stdout.is_empty() {
                                    let (a, b) = stdout.split_at(stdout.len() / 2);
                                    [a, b]
                                        .map(|part| {
                                            Output {
                                                data: part.as_bytes().to_vec(),
                                            }
                                            .encode_to_vec()
                                        })
                                        .into()
                                } else {
                                    vec![]
                                })
                            }
                            "TaskExecWait" => grpc(vec![Exit { code: Some(0) }.encode_to_vec()]),
                            _ => panic!("unexpected router method {method}"),
                        };
                    }
                    assert_eq!(service, "modal.client.ModalClient");
                    assert_eq!(
                        (
                            headers["x-modal-token-id"].to_str().unwrap(),
                            headers["x-modal-token-secret"].to_str().unwrap()
                        ),
                        ("ak-fixture", "as-fixture")
                    );
                    assert_eq!(headers["x-modal-client-version"], "1.1.4");
                    let sandbox_call = method.starts_with("Sandbox");
                    assert_eq!(
                        headers.get("x-modal-auth-token").is_some(),
                        sandbox_call,
                        "{method}"
                    );
                    if sandbox_call {
                        assert_eq!(headers["x-modal-auth-token"], "auth-1");
                    }
                    match method {
                        "AuthTokenGet" => grpc(vec![
                            Token {
                                token: "auth-1".into(),
                            }
                            .encode_to_vec(),
                        ]),
                        "AppGetOrCreate" => {
                            assert_eq!(Id::decode(request).unwrap().id, "tilde-sandbox-tools");
                            grpc(vec![Id { id: "ap-1".into() }.encode_to_vec()])
                        }
                        // The image is still building: its log stream ends with the result.
                        "ImageGetOrCreate" => grpc(vec![Id { id: "im-1".into() }.encode_to_vec()]),
                        "ImageJoinStreaming" => grpc(vec![
                            ImageJoin {
                                result: None,
                                entry_id: "log-1".into(),
                            }
                            .encode_to_vec(),
                            ImageJoin {
                                result: Some(ImageResult { status: 1 }),
                                entry_id: String::new(),
                            }
                            .encode_to_vec(),
                        ]),
                        "SandboxCreateV2" => {
                            let create = SandboxCreate::decode(request).unwrap();
                            assert_eq!(create.app_id, "ap-1");
                            assert_eq!(
                                create.definition,
                                Some(SandboxDefinition {
                                    image_id: "im-1".into(),
                                    timeout_secs: 300
                                })
                            );
                            grpc(vec![
                                Created {
                                    sandbox_id: "sb-1".into(),
                                    task_id: "ta-1".into(),
                                }
                                .encode_to_vec(),
                            ])
                        }
                        "SandboxGetTaskIdV2" => {
                            assert_eq!(Id::decode(request).unwrap().id, "sb-1");
                            grpc(vec![Id { id: "ta-1".into() }.encode_to_vec()])
                        }
                        "SandboxGetCommandRouterAccess" => grpc(vec![
                            RouterAccess {
                                jwt: "jwt-1".into(),
                                url: router_url,
                            }
                            .encode_to_vec(),
                        ]),
                        _ => panic!("unexpected Modal method {method}"),
                    }
                }
            },
        );
        axum::serve(listener, app).await.unwrap()
    });
    (origin, server)
}

#[tokio::test]
async fn aws_and_modal_tools_reach_their_upstreams_through_an_agent() {
    let aws_calls: Arc<Mutex<Vec<Value>>> = Default::default();
    let (aws, aws_server) = aws_upstream(aws_calls.clone()).await;
    let files: Arc<Mutex<BTreeMap<String, String>>> = Arc::new(Mutex::new(BTreeMap::from([(
        "/work/app.py".to_owned(),
        "def main():\n    return 1\n".to_owned(),
    )])));
    let commands: Arc<Mutex<Vec<Vec<String>>>> = Default::default();
    let (modal, modal_server) = modal_upstream(files.clone(), commands.clone()).await;
    let fx = Fixture::new(
        72,
        Endpoints(BTreeMap::from([
            ("aws_mcp".into(), format!("{aws}/mcp")),
            ("modal_api".into(), modal),
        ])),
        all_tools(),
    )
    .await;
    let aws_connection = connection(
        &fx,
        "aws",
        "iam",
        &[
            ("access_key_id", "AKIDFIXTURE"),
            ("secret_access_key", "secret-fixture"),
            ("session_token", "session-fixture"),
            ("region", "eu-west-1"),
        ],
    )
    .await;
    let modal_connection = connection(
        &fx,
        "modal",
        "api",
        &[
            ("api_key_id", "ak-fixture"),
            ("api_key_secret", "as-fixture"),
            ("workspace_id", "ac-fixture"),
        ],
    )
    .await;

    let tools = fx.chat.tools.clone().unwrap();
    assert_eq!(tools.provider_tools(aws_connection).await.unwrap().len(), 9);
    assert_eq!(
        tools.provider_tools(modal_connection).await.unwrap().len(),
        10
    );
    assert_eq!(
        fx.use_tools(aws_connection, &["aws_list_regions"]).await,
        "aws"
    );
    assert_eq!(
        fx.use_tools(
            modal_connection,
            &["create_sandbox", "exec_command", "apply_patch"]
        )
        .await,
        "modal"
    );
    let listed = fx.tools().await;
    let regions = listed
        .iter()
        .find(|t| t["name"] == "aws.aws_list_regions")
        .unwrap();
    assert_eq!(
        (
            regions["providerId"].clone(),
            regions["annotations"]["readOnly"].clone()
        ),
        (json!("aws"), json!(true))
    );

    // AWS: a JSON text answer becomes the output; the connection's region rides in `_meta`.
    assert_eq!(
        fx.output("aws.aws_list_regions", json!({})).await,
        json!({"regions":["eu-west-1"]})
    );
    assert_eq!(
        *aws_calls.lock().unwrap(),
        vec![
            json!({"name":"aws___list_regions","arguments":{},"_meta":{"AWS_REGION":"eu-west-1"}})
        ]
    );

    // Modal: app, image build and sandbox, then commands and a patch through the router.
    assert_eq!(
        fx.output("modal.create_sandbox", json!({})).await,
        json!({"sandbox_id":"sb-1","task_id":"ta-1"})
    );
    assert_eq!(
        fx.output(
            "modal.exec_command",
            json!({"sandbox_id":"sb-1","cmd":"python3 --version"})
        )
        .await,
        json!({"exit_code":0,"stdout":"ran: python3 --version\n","stderr":"","timed_out":false})
    );
    let patch = "*** Begin Patch\n*** Update File: /work/app.py\n@@ def main():\n-    return 1\n+    return 2\n*** Add File: /work/notes/todo.txt\n+ship it\n*** End Patch";
    assert_eq!(
        fx.output(
            "modal.apply_patch",
            json!({"sandbox_id":"sb-1","patch":patch})
        )
        .await,
        json!({"added":["/work/notes/todo.txt"],"modified":["/work/app.py"],"deleted":[]})
    );
    assert_eq!(
        *files.lock().unwrap(),
        BTreeMap::from([
            (
                "/work/app.py".to_owned(),
                "def main():\n    return 2\n".to_owned()
            ),
            ("/work/notes/todo.txt".to_owned(), "ship it\n".to_owned()),
        ])
    );
    assert!(
        commands.lock().unwrap()[3][1].starts_with("mkdir -p -- '/work/notes' && "),
        "parents are created for new files"
    );
    let failed = fx
        .invoke(
            "modal.apply_patch",
            json!({"sandbox_id":"sb-1","patch":"*** Begin Patch\n*** Update File: /work/app.py\n@@\n-missing line\n+x\n*** End Patch"}),
        )
        .await;
    assert!(
        failed["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("expected lines not found")),
        "{failed}"
    );

    aws_server.abort();
    modal_server.abort();
    fx.close().await;
}
