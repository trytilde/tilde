//! The loopback HTTP API that processes in the VM (git credential helpers, scripts, the agent's
//! own commands) use to reach the blueprint's tools, and the `tools` and `call` commands that
//! talk to it. Browsers are kept out: any `Origin` header is refused, and so is any `Host` that
//! is not this loopback address, which stops DNS-rebinding pages too.
use super::{Daemon, wire};
use anyhow::{Context, Result, bail};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use connectrpc::ErrorCode as Code;
use serde_json::{Value, json};
use std::{io::IsTerminal, net::SocketAddr, sync::Arc, sync::atomic::Ordering, time::Duration};

pub const DEFAULT_LISTEN: &str = "127.0.0.1:4790";

pub(super) async fn serve(
    daemon: Arc<Daemon>,
    address: SocketAddr,
) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .with_context(|| format!("Could not listen on {address}"))?;
    let bound = listener.local_addr()?;
    let port = bound.port();
    let app = Router::new()
        .route("/v1/health", get(health))
        .route("/v1/tools", get(list))
        .route("/v1/tools/{name}", post(invoke))
        .layer(middleware::from_fn(move |request, next| {
            guard(port, request, next)
        }))
        .with_state(daemon);
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Ok((bound, handle))
}

async fn guard(port: u16, request: Request, next: Next) -> Response {
    if request.headers().contains_key(header::ORIGIN) {
        return error(StatusCode::FORBIDDEN, "Browser requests are not accepted");
    }
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|host| host.to_str().ok())
        .unwrap_or_default();
    let allowed = ["127.0.0.1", "localhost", "[::1]"]
        .iter()
        .any(|name| host == format!("{name}:{port}"));
    if !allowed {
        return error(StatusCode::FORBIDDEN, "Unexpected Host header");
    }
    next.run(request).await
}

fn error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

fn not_connected() -> Response {
    error(
        StatusCode::SERVICE_UNAVAILABLE,
        "This sandbox has not connected to Tilde yet",
    )
}

fn failure(error: connectrpc::ConnectError) -> Response {
    let status = match error.code {
        Code::InvalidArgument => StatusCode::BAD_REQUEST,
        Code::NotFound => StatusCode::NOT_FOUND,
        Code::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::BAD_GATEWAY,
    };
    let message = error
        .message
        .unwrap_or_else(|| format!("{:?}", error.code).to_lowercase());
    self::error(status, &message)
}

async fn health(State(daemon): State<Arc<Daemon>>) -> Response {
    Json(json!({
        "connected": daemon.connected.load(Ordering::Relaxed),
        "sandbox_id": *daemon.sandbox_id.lock().unwrap(),
    }))
    .into_response()
}

async fn list(State(daemon): State<Arc<Daemon>>) -> Response {
    if daemon.sandbox_id.lock().unwrap().is_none() {
        return not_connected();
    }
    let response = match daemon
        .client
        .list_tools_with_options(wire::ListToolsRequest::default(), daemon.options())
        .await
    {
        Ok(response) => response.into_owned(),
        Err(error) => return failure(error),
    };
    let schema = |json: &str| serde_json::from_str::<Value>(json).ok();
    let tools: Vec<Value> = response
        .tools
        .iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "summary": tool.summary,
                "input_schema": schema(&tool.input_schema_json).unwrap_or_else(|| json!({})),
                "output_schema": schema(&tool.output_schema_json),
            })
        })
        .collect();
    Json(json!({ "tools": tools })).into_response()
}

/// Names the operation the calling command runs under (TILDE_SANDBOX_OPERATION).
const OPERATION_HEADER: &str = "x-tilde-sandbox-operation";

async fn invoke(
    State(daemon): State<Arc<Daemon>>,
    Path(name): Path<String>,
    headers: header::HeaderMap,
    body: Bytes,
) -> Response {
    let input = if body.iter().all(u8::is_ascii_whitespace) {
        "{}".to_string()
    } else {
        match serde_json::from_slice::<Value>(&body) {
            Ok(input) => input.to_string(),
            Err(_) => return error(StatusCode::BAD_REQUEST, "The request body is not JSON"),
        }
    };
    if daemon.sandbox_id.lock().unwrap().is_none() {
        return not_connected();
    }
    let request = wire::InvokeToolRequest {
        name,
        input_json: input,
        operation_id: headers
            .get(OPERATION_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned),
        ..Default::default()
    };
    match daemon
        .client
        .invoke_tool_with_options(request, daemon.options())
        .await
    {
        Ok(response) => (
            [(header::CONTENT_TYPE, "application/json")],
            response.into_owned().output_json,
        )
            .into_response(),
        Err(error) => failure(error),
    }
}

fn http() -> Result<reqwest::Client> {
    // No proxy: a VM-wide HTTP_PROXY must not capture loopback calls.
    Ok(reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(300))
        .build()?)
}

/// Turn a local API response into its JSON, or its `error` message.
async fn answer(response: reqwest::Response) -> Result<Value> {
    let status = response.status();
    let body: Value = response
        .json()
        .await
        .context("The sandbox process answered with something other than JSON")?;
    if !status.is_success() {
        bail!(
            "{}",
            body["error"].as_str().unwrap_or("The tool call failed")
        );
    }
    Ok(body)
}

fn unreachable(listen: SocketAddr) -> String {
    format!("Could not reach `tilde sandbox connect` on {listen}; is it running?")
}

pub async fn tools(listen: SocketAddr, raw: bool) -> Result<()> {
    let response = http()?
        .get(format!("http://{listen}/v1/tools"))
        .send()
        .await
        .with_context(|| unreachable(listen))?;
    let body = answer(response).await?;
    if raw {
        println!("{body}");
        return Ok(());
    }
    for tool in body["tools"].as_array().into_iter().flatten() {
        let summary = [&tool["summary"], &tool["description"]]
            .into_iter()
            .filter_map(Value::as_str)
            .find(|text| !text.is_empty())
            .unwrap_or_default();
        println!("{}  {summary}", tool["name"].as_str().unwrap_or_default());
    }
    Ok(())
}

pub async fn call(listen: SocketAddr, name: &str, input: Option<String>) -> Result<()> {
    let input = match input {
        Some(input) => input,
        None if !std::io::stdin().is_terminal() => {
            let mut input = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut input)
                .context("Could not read the input from stdin")?;
            input
        }
        None => String::new(),
    };
    let mut request = http()?
        .post(format!("http://{listen}/v1/tools/{name}"))
        .header(header::CONTENT_TYPE, "application/json");
    if let Ok(operation) = std::env::var(super::ops::OPERATION_ENV) {
        request = request.header(OPERATION_HEADER, operation);
    }
    let response = request
        .body(input)
        .send()
        .await
        .with_context(|| unreachable(listen))?;
    println!("{}", answer(response).await?);
    Ok(())
}
