//! Tool calls that outlast a proxy's idle timeout. Production sits behind a load balancer that
//! closes a connection once no bytes moved for 60 s and speaks HTTP/1.1 to Tilde; here a TCP
//! proxy closes after 2 s idle, the client speaks HTTP/1.1 through it, and the server's heartbeat
//! is shortened to 500 ms. A 4 s tool call (a Tavily call against a slow fake API) then succeeds
//! over InvokeToolStream and is cut over the silent InvokeTool.
mod common;
use common::invocation::{Fixture, all_tools, drain, framed};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use std::time::Duration;
use tilde::connections::catalog::Endpoints;
use uuid::Uuid;

const IDLE: Duration = Duration::from_secs(2);
const TOOL: Duration = Duration::from_secs(4);

/// A Tavily API whose `/usage` answers after `TOOL`.
async fn slow_tavily() -> String {
    let app = axum::Router::new().route(
        "/usage",
        axum::routing::get(|| async {
            tokio::time::sleep(TOOL).await;
            axum::Json(json!({"key": {"usage": 7, "limit": 1000}}))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    origin
}

/// The call's Connect frames, or `None` when the connection was cut before the response ended.
async fn call(
    proxy: &str,
    token: &SecretString,
    method: &str,
    body: reqwest::Body,
) -> Option<Vec<Value>> {
    let response = reqwest::Client::builder()
        .http1_only()
        .build()
        .unwrap()
        .post(format!("{proxy}/tilde.runtime.v1.ChatService/{method}"))
        .bearer_auth(token.expose_secret())
        .header("content-type", "application/connect+json")
        .header("connect-protocol-version", "1")
        .body(body)
        .send()
        .await
        .ok()?;
    let mut bytes = response.bytes().await.ok()?.to_vec();
    let frames = drain(&mut bytes);
    (!frames.is_empty()).then_some(frames)
}

fn frame(call: Uuid, name: &str, sequence: u32, extra: Value) -> Vec<u8> {
    let mut frame = json!({"name": name, "callId": call.to_string(), "sequence": sequence});
    frame
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    framed(frame)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn long_tool_calls_survive_an_idle_timeout_proxy_only_with_heartbeats() {
    tilde::set_heartbeat_interval(Duration::from_millis(500));
    let tavily = slow_tavily().await;
    let fx = Fixture::new(
        91,
        Endpoints([("tavily_api".to_owned(), tavily)].into()),
        all_tools(),
    )
    .await;
    let connection = Uuid::new_v4();
    let setup = async {
        fx.connections
            .start(connection, "Search", "tavily", "api", &[])
            .await
            .unwrap();
        fx.db
            .pool
            .get()
            .await
            .unwrap()
            .execute(
                "UPDATE connections SET status='ready' WHERE id=$1",
                &[&connection],
            )
            .await
            .unwrap();
        fx.connections
            .replace_value(connection, "api_key", &SecretString::from("key"))
            .await
            .unwrap();
    };
    fx.within(setup).await;
    let usage = format!("{}.usage", fx.use_tools(connection, &["usage"]).await);
    let proxy = common::idle_proxy(fx.origin.trim_start_matches("http://").to_owned(), IDLE).await;

    // Heartbeats every 500 ms keep the connection open through the 4 s call; the result follows.
    let started = std::time::Instant::now();
    let frames = call(
        &proxy,
        &fx.token,
        "InvokeToolStream",
        frame(
            Uuid::new_v4(),
            &usage,
            0,
            json!({"inputJson": "{}", "finish": true}),
        )
        .into(),
    )
    .await
    .expect("the streamed call completes through the proxy");
    assert!(started.elapsed() >= TOOL);
    let heartbeats = frames
        .iter()
        .filter(|f| f.get("heartbeat").is_some())
        .count();
    assert!(heartbeats >= 4, "{frames:?}");
    let result = frames
        .iter()
        .find_map(|f| f["result"]["outputJson"].as_str())
        .unwrap_or_else(|| panic!("{frames:?}"));
    assert_eq!(
        serde_json::from_str::<Value>(result).unwrap()["key"]["usage"],
        7
    );

    // The same call over the silent InvokeTool: the proxy cuts it after 2 s.
    let started = std::time::Instant::now();
    let silent = call(
        &proxy,
        &fx.token,
        "InvokeTool",
        frame(
            Uuid::new_v4(),
            &usage,
            0,
            json!({"inputJson": "{}", "finish": true}),
        )
        .into(),
    )
    .await;
    assert!(silent.is_none(), "{silent:?}");
    assert!(started.elapsed() < TOOL);

    // Streamed input over HTTP/1.1: chunks of a native message arrive a second apart while
    // heartbeats flow back, for longer than the proxy's idle timeout in all.
    let message = Uuid::new_v4();
    let chunks = async_stream::stream! {
        yield Ok::<_, std::io::Error>(frame(message, "sendMessage", 0, json!({"inputJson": r#"{"text":""}"#, "finish": false})));
        for (sequence, text) in [(1, "Long "), (2, "work "), (3, "done")] {
            tokio::time::sleep(Duration::from_secs(1)).await;
            yield Ok(frame(message, "sendMessage", sequence, json!({"chunkJson": json!({"textDelta": text}).to_string()})));
        }
        yield Ok(frame(message, "sendMessage", 4, json!({"finish": true})));
    };
    let frames = call(
        &proxy,
        &fx.token,
        "InvokeToolStream",
        reqwest::Body::wrap_stream(chunks),
    )
    .await
    .expect("the streamed message completes through the proxy");
    let result = frames
        .iter()
        .find_map(|f| f["result"]["outputJson"].as_str())
        .unwrap_or_else(|| panic!("{frames:?}"));
    assert_eq!(
        serde_json::from_str::<Value>(result).unwrap()["text"],
        "Long work done"
    );
    fx.close().await;
}
