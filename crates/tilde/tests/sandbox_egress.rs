//! Sandbox egress end to end: a sandbox process enrolls and receives its egress token and
//! wiring, then real `git` clones and pushes through the gateway route to a git smart-HTTP
//! upstream (`git http-backend`) and plain HTTP calls go to the API, both with the connection's
//! credential attached by the gateway. Pushes outside the grant's branches, deletes and
//! non-fast-forward updates are refused before anything reaches upstream (from full and shallow
//! clones alike), as are denied API
//! paths; the sandbox never sees the connection's credential and upstream never sees the
//! sandbox's token.
mod common;
use axum::{
    Router,
    body::Bytes,
    extract::{Request, State},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
};
use common::invocation::{drain, framed};
use common::live::Live;
use futures::StreamExt;
use secrecy::SecretString;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
};
use tilde::connections::catalog::Endpoints;
use uuid::Uuid;

const UPSTREAM_TOKEN: &str = "github_pat_upstream_fixture";

#[derive(Default)]
struct Seen {
    /// `METHOD path` and the authorization header of every upstream request.
    requests: Vec<(String, String)>,
}
struct Upstream {
    root: PathBuf,
    seen: Mutex<Seen>,
}

/// GitHub's two hosts in one: git smart-HTTP through `git http-backend` under `root`, and a
/// REST API that answers any request.
async fn upstream(root: PathBuf) -> (String, Arc<Upstream>) {
    let state = Arc::new(Upstream {
        root,
        seen: Mutex::default(),
    });
    let app = Router::new().fallback(serve).with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (origin, state)
}
async fn serve(State(upstream): State<Arc<Upstream>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let path = parts.uri.path().to_owned();
    let auth = parts
        .headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    upstream
        .seen
        .lock()
        .unwrap()
        .requests
        .push((format!("{} {path}", parts.method), auth));
    let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    if path.contains(".git/") {
        return cgi(
            &upstream.root,
            &parts.method,
            &path,
            parts.uri.query(),
            &parts.headers,
            body,
        );
    }
    axum::Json(json!({"full_name":"acme/widgets","path":path})).into_response()
}
fn cgi(
    root: &Path,
    method: &Method,
    path: &str,
    query: Option<&str>,
    headers: &HeaderMap,
    body: Bytes,
) -> Response {
    use std::io::Write;
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_owned()
    };
    let mut child = Command::new("git")
        .arg("http-backend")
        .env("GIT_PROJECT_ROOT", root)
        .env("GIT_HTTP_EXPORT_ALL", "1")
        .env("REMOTE_USER", "fixture")
        .env("PATH_INFO", path)
        .env("QUERY_STRING", query.unwrap_or_default())
        .env("REQUEST_METHOD", method.as_str())
        .env("CONTENT_TYPE", header("content-type"))
        .env("CONTENT_LENGTH", body.len().to_string())
        .env("HTTP_CONTENT_ENCODING", header("content-encoding"))
        .env("GIT_PROTOCOL", header("git-protocol"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&body).unwrap();
    let output = child.wait_with_output().unwrap();
    let split = output
        .stdout
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| (i, 4))
        .or_else(|| {
            output
                .stdout
                .windows(2)
                .position(|w| w == b"\n\n")
                .map(|i| (i, 2))
        })
        .unwrap();
    let head = String::from_utf8_lossy(&output.stdout[..split.0]).to_string();
    let mut response = Response::builder();
    for line in head.lines() {
        let (name, value) = line.split_once(':').unwrap();
        if name.eq_ignore_ascii_case("status") {
            let code: u16 = value.trim()[..3].parse().unwrap();
            response = response.status(code);
        } else {
            response = response.header(name.trim(), value.trim());
        }
    }
    response
        .body(axum::body::Body::from(
            output.stdout[split.0 + split.1..].to_vec(),
        ))
        .unwrap()
}

/// A directory removed when the test ends.
struct Scratch(PathBuf);
impl Scratch {
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn git(dir: &Path, home: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("HOME", home)
        .env("GIT_CONFIG_GLOBAL", home.join(".gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_AUTHOR_NAME", "Agent")
        .env("GIT_AUTHOR_EMAIL", "agent@tilde.test")
        .env("GIT_COMMITTER_NAME", "Agent")
        .env("GIT_COMMITTER_EMAIL", "agent@tilde.test")
        .output()
        .unwrap()
}
fn ok(output: std::process::Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}
fn refused(output: std::process::Output, why: &str) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(!output.status.success(), "{why} was accepted: {stderr}");
    stderr
}

// `git` runs as blocking child processes while the route and upstream serve it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sandbox_clones_and_pushes_through_egress_within_its_grant() {
    let dir = Scratch(std::env::temp_dir().join(format!("tilde-egress-{}", Uuid::new_v4())));
    let root = dir.path().join("upstream");
    let bare = root.join("acme/widgets.git");
    std::fs::create_dir_all(&bare).unwrap();
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(home.join(".gitconfig"), "").unwrap();
    ok(git(
        &bare,
        &home,
        &["init", "--bare", "--initial-branch=main", "."],
    ));
    ok(git(&bare, &home, &["config", "http.receivepack", "true"]));
    ok(git(
        &bare,
        &home,
        &["config", "receive.advertisePushOptions", "true"],
    ));
    // Records the push options upstream received, to show they pass through the gateway.
    let hook = bare.join("hooks/pre-receive");
    std::fs::write(
        &hook,
        "#!/bin/sh\ni=0\nwhile [ $i -lt \"${GIT_PUSH_OPTION_COUNT:-0}\" ]; do eval echo \"\\$GIT_PUSH_OPTION_$i\"; i=$((i+1)); done >> \"$GIT_DIR/push-options\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&hook, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let seed = dir.path().join("seed");
    std::fs::create_dir_all(&seed).unwrap();
    ok(git(&seed, &home, &["init", "--initial-branch=main", "."]));
    std::fs::write(seed.join("README.md"), "widgets\n").unwrap();
    ok(git(&seed, &home, &["add", "."]));
    ok(git(&seed, &home, &["commit", "-m", "Initial"]));
    ok(git(&seed, &home, &["push", bare.to_str().unwrap(), "main"]));

    let (origin, upstream) = upstream(root).await;
    let live = Live::with_endpoints(
        73,
        "github",
        "pat",
        &[("token", UPSTREAM_TOKEN.into())],
        Endpoints(BTreeMap::from([
            ("github_git".into(), origin.clone()),
            ("github_api".into(), origin.clone()),
        ])),
    )
    .await;
    let fx = &live.fx;
    let sandboxes = fx.chat.tools.as_ref().unwrap().sandboxes.clone();
    let pool = fx.db.pool.clone();
    let enrollment = "tilde_sbx_enrollment_fixture";
    let (blueprint, sandbox) = fx
        .within(async {
            let e2b = Uuid::new_v4();
            fx.connections
                .start(e2b, "e2b", "e2b", "api", &[])
                .await
                .unwrap();
            let pg = pool.get().await.unwrap();
            pg.execute("UPDATE connections SET status='ready' WHERE id=$1", &[&e2b])
                .await
                .unwrap();
            fx.connections
                .replace_value(e2b, "api_key", &SecretString::from("e2b_fixture"))
                .await
                .unwrap();
            let blueprint = sandboxes
                .create_blueprint(
                    "factory",
                    e2b,
                    "tilde-base",
                    "thread",
                    Default::default(),
                )
                .await
                .unwrap();
            sandboxes
                .set_egress(
                    blueprint.id,
                    "github",
                    live.connection,
                    &json!({"repositories":["acme/widgets"]}),
                    true,
                )
                .await
                .unwrap();
            // A variable that references a secret gets the secret's current value at connect.
            let secret = tilde::secrets::Secrets::new(pool.clone(), fx.crypto.clone())
                .set(
                    None,
                    "NPM_TOKEN",
                    Some(SecretString::from("npm_fixture")),
                    "",
                )
                .await
                .unwrap();
            pg.execute(
                "INSERT INTO sandbox_blueprint_env(blueprint_id,name,secret_id) VALUES($1,'NPM_TOKEN',$2)",
                &[&blueprint.id, &secret.id],
            )
            .await
            .unwrap();
            let sandbox = Uuid::new_v4();
            let hash = <sha2::Sha256 as sha2::Digest>::digest(enrollment.as_bytes());
            pg.execute(
                "INSERT INTO sandboxes(id,blueprint_id,reuse,connection_id,agent_id,thread_id,status,enrollment_hash) VALUES($1,$2,'thread',$3,$4,$5,'running',$6)",
                &[&sandbox, &blueprint.id, &e2b, &fx.agent, &fx.thread, &hash.to_vec()],
            )
            .await
            .unwrap();
            (blueprint.id, sandbox)
        })
        .await;
    let _ = (blueprint, sandbox);

    // The process enrolls and receives its egress token and wiring, never the credential.
    let connected = reqwest::Client::new()
        .post(format!(
            "{}/tilde.sandbox.v1.SandboxService/Connect",
            fx.origin
        ))
        .bearer_auth(enrollment)
        .header("content-type", "application/connect+json")
        .header("connect-protocol-version", "1")
        .body(framed(json!({})))
        .send()
        .await
        .unwrap();
    assert_eq!(connected.status(), 200);
    let mut stream = connected.bytes_stream();
    let mut buffer = vec![];
    let registered = loop {
        buffer.extend_from_slice(&stream.next().await.unwrap().unwrap());
        if let Some(frame) = drain(&mut buffer).into_iter().next() {
            break frame["registered"].clone();
        }
    };
    let registration = registered.to_string();
    assert!(!registration.contains(UPSTREAM_TOKEN));
    let token = registered["egressToken"].as_str().unwrap().to_owned();
    assert!(
        registered["env"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["name"] == "NPM_TOKEN" && v["value"] == "npm_fixture"),
        "{registration}"
    );
    let env: BTreeMap<String, String> = registered["egressEnv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            (
                v["name"].as_str().unwrap().into(),
                v["value"].as_str().unwrap().into(),
            )
        })
        .collect();
    assert!(env["TILDE_EGRESS_GITHUB_URL"].ends_with("/sandboxes/egress/github"));
    assert_eq!(env["GITHUB_API_URL"], env["TILDE_EGRESS_GITHUB_URL"]);
    assert_eq!(env["GITHUB_TOKEN"], "{token}");
    assert_eq!(registered["git"][0]["insteadOf"], "https://github.com/");
    assert!(
        registered["git"][0]["url"]
            .as_str()
            .unwrap()
            .ends_with("/sandboxes/egress/github/")
    );

    // What `tilde sandbox connect` writes: the rewrite to the route and a credential helper
    // answering with the egress token.
    let route = format!("{}/sandboxes/egress/github", fx.origin);
    std::fs::write(
        home.join(".gitconfig"),
        format!(
            "[url \"{route}/\"]\n\tinsteadOf = https://github.com/\n[credential \"{}\"]\n\thelper = \"!f() {{ echo username=x-access-token; echo password={token}; }}; f\"\n",
            fx.origin
        ),
    )
    .unwrap();
    let work = dir.path().join("work");
    ok(git(
        dir.path(),
        &home,
        &["clone", "https://github.com/acme/widgets.git", "work"],
    ));
    assert_eq!(
        std::fs::read_to_string(work.join("README.md")).unwrap(),
        "widgets\n"
    );

    // A push to a granted branch reaches upstream; others, deletes and rewrites do not.
    std::fs::write(work.join("feature.txt"), "one\n").unwrap();
    ok(git(&work, &home, &["add", "."]));
    ok(git(&work, &home, &["commit", "-m", "Feature"]));
    ok(git(
        &work,
        &home,
        &["push", "origin", "HEAD:refs/heads/factory/feature"],
    ));
    let first = ok(git(&bare, &home, &["rev-parse", "factory/feature"]));
    assert_eq!(first, ok(git(&work, &home, &["rev-parse", "HEAD"])));
    std::fs::write(work.join("feature.txt"), "two\n").unwrap();
    ok(git(&work, &home, &["commit", "-am", "More"]));
    ok(git(
        &work,
        &home,
        &["push", "origin", "HEAD:refs/heads/factory/feature"],
    ));
    let second = ok(git(&work, &home, &["rev-parse", "HEAD"]));

    let stderr = refused(
        git(&work, &home, &["push", "origin", "HEAD:refs/heads/main"]),
        "main",
    );
    assert!(stderr.contains("403"), "{stderr}");
    refused(
        git(&work, &home, &["push", "origin", "HEAD:refs/heads/release"]),
        "an ungranted branch",
    );
    refused(
        git(
            &work,
            &home,
            &["push", "origin", ":refs/heads/factory/feature"],
        ),
        "a delete",
    );
    ok(git(&work, &home, &["reset", "--hard", "HEAD~1"]));
    std::fs::write(work.join("feature.txt"), "rewritten\n").unwrap();
    ok(git(&work, &home, &["commit", "-am", "Rewrite"]));
    refused(
        git(
            &work,
            &home,
            &[
                "push",
                "--force",
                "origin",
                "HEAD:refs/heads/factory/feature",
            ],
        ),
        "a force push",
    );
    assert_eq!(
        ok(git(&bare, &home, &["rev-parse", "factory/feature"])),
        second
    );
    assert_eq!(
        ok(git(&bare, &home, &["rev-parse", "main"])),
        ok(git(&seed, &home, &["rev-parse", "HEAD"]))
    );

    // A shallow clone's push starts with `shallow` lines; it is read and checked like any other.
    std::fs::write(seed.join("README.md"), "widgets, again\n").unwrap();
    ok(git(&seed, &home, &["commit", "-am", "Second"]));
    ok(git(&seed, &home, &["push", bare.to_str().unwrap(), "main"]));
    let shallow = dir.path().join("shallow");
    ok(git(
        dir.path(),
        &home,
        &[
            "clone",
            "--depth",
            "1",
            "https://github.com/acme/widgets.git",
            "shallow",
        ],
    ));
    assert!(shallow.join(".git/shallow").exists());
    std::fs::write(shallow.join("shallow.txt"), "one\n").unwrap();
    ok(git(&shallow, &home, &["add", "."]));
    ok(git(
        &shallow,
        &home,
        &["commit", "-m", "From a shallow clone"],
    ));
    ok(git(
        &shallow,
        &home,
        &["push", "origin", "HEAD:refs/heads/factory/shallow"],
    ));
    assert_eq!(
        ok(git(&bare, &home, &["rev-parse", "factory/shallow"])),
        ok(git(&shallow, &home, &["rev-parse", "HEAD"]))
    );
    refused(
        git(&shallow, &home, &["push", "origin", "HEAD:refs/heads/main"]),
        "main from a shallow clone",
    );
    refused(
        git(
            &shallow,
            &home,
            &["push", "origin", ":refs/heads/factory/shallow"],
        ),
        "a delete from a shallow clone",
    );
    ok(git(&shallow, &home, &["reset", "--hard", "HEAD~1"]));
    std::fs::write(shallow.join("shallow.txt"), "rewritten\n").unwrap();
    ok(git(&shallow, &home, &["add", "."]));
    ok(git(&shallow, &home, &["commit", "-m", "Rewrite"]));
    refused(
        git(
            &shallow,
            &home,
            &[
                "push",
                "--force",
                "origin",
                "HEAD:refs/heads/factory/shallow",
            ],
        ),
        "a force push from a shallow clone",
    );
    // Push options follow the commands after their own flush.
    ok(git(
        &shallow,
        &home,
        &[
            "push",
            "-o",
            "ci.skip",
            "origin",
            "HEAD:refs/heads/factory/options",
        ],
    ));
    assert_eq!(
        std::fs::read_to_string(bare.join("push-options")).unwrap(),
        "ci.skip\n"
    );
    // Above `http.postBuffer` (1 MiB) git first POSTs a flush-only probe, then the push.
    let large: String = (0..120_000)
        .map(|_| Uuid::new_v4().simple().to_string())
        .collect();
    std::fs::write(shallow.join("large.txt"), large).unwrap();
    ok(git(&shallow, &home, &["add", "."]));
    ok(git(&shallow, &home, &["commit", "-m", "Large"]));
    ok(git(
        &shallow,
        &home,
        &["push", "origin", "HEAD:refs/heads/factory/options"],
    ));
    assert_eq!(
        ok(git(&bare, &home, &["rev-parse", "factory/options"])),
        ok(git(&shallow, &home, &["rev-parse", "HEAD"]))
    );

    // REST calls on the allowlist pass with the connection's credential; denied ones stop here.
    let http = reqwest::Client::new();
    let read = http
        .get(format!("{route}/repos/acme/widgets"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(read.status(), 200);
    let body: Value = read.json().await.unwrap();
    assert_eq!(body["full_name"], "acme/widgets");
    for (method, path, status) in [
        (Method::DELETE, "/repos/acme/widgets", StatusCode::FORBIDDEN),
        (
            Method::GET,
            "/repos/acme/widgets/hooks",
            StatusCode::FORBIDDEN,
        ),
        (Method::GET, "/repos/mallory/secrets", StatusCode::FORBIDDEN),
        (
            Method::GET,
            "/repos/acme/widgets/../../orgs/acme",
            StatusCode::FORBIDDEN,
        ),
        (
            Method::PUT,
            "/orgs/acme/members/agent",
            StatusCode::FORBIDDEN,
        ),
    ] {
        let response = http
            .request(method.clone(), format!("{route}{path}"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), status, "{method} {path}");
    }
    let anonymous = http
        .get(format!("{route}/repos/acme/widgets"))
        .send()
        .await
        .unwrap();
    assert_eq!(anonymous.status(), 401);
    let wrong = http
        .get(format!(
            "{}/sandboxes/egress/slack/api/chat.postMessage",
            fx.origin
        ))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(wrong.status(), 401);

    let seen = std::mem::take(&mut *upstream.seen.lock().unwrap());
    assert!(
        seen.requests.iter().all(|(_, auth)| !auth.contains(&token)),
        "the sandbox's token reached upstream"
    );
    let api: Vec<_> = seen
        .requests
        .iter()
        .filter(|(route, _)| !route.contains(".git"))
        .collect();
    assert_eq!(api.len(), 1, "{api:?}");
    assert_eq!(api[0].0, "GET /repos/acme/widgets");
    assert_eq!(api[0].1, format!("Bearer {UPSTREAM_TOKEN}"));
    // Git authenticates with the token as Basic password; refused pushes never reached it.
    assert!(seen.requests.iter().any(|(route, auth)| route.ends_with("/git-receive-pack")
        && auth.starts_with("Basic ")));
    let pushes = seen
        .requests
        .iter()
        .filter(|(route, _)| route == "POST /acme/widgets.git/git-receive-pack")
        .count();
    // Two pushes from the full clone; from the shallow one a push, push options, the probe and
    // the large push.
    assert_eq!(pushes, 6);
    live.close().await;
}
