//! A local agent's tools as an MCP server (streamable HTTP, JSON responses, no session): the
//! one tool protocol every harness speaks. It lists the agent's tools on ready sources and
//! runs them as the gateway does for an invocation, with the agent as the caller and no
//! thread. Adding a source to the agent is the grant: a local agent has no capabilities.
//!
//! Not served: sandbox tools (they run in an invocation's sandbox) and personal tools.
//!
//! Known gap: a call leaves no record of its own here. `chat_tool_calls` and tool spans belong
//! to invocations; a coding harness reports the call with its session, but a desktop app
//! reports nothing, so its calls are visible only in the tool provider's own logs.
use super::Caller;
use crate::chat::Chat;
use crate::tools::{Callee, Owner};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

const MAX_REQUEST: usize = 1024 * 1024;

/// Catalog names are `{source}.{tool}`; MCP clients accept only letters, digits, `_` and `-`.
fn name(catalog: &str) -> String {
    catalog
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
fn failure(id: &Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}

pub(super) async fn handle(
    chat: &Chat,
    caller: &Caller,
    request: axum::extract::Request,
) -> Response {
    if request.method() != http::Method::POST {
        return http::StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    let Ok(body) = axum::body::to_bytes(request.into_body(), MAX_REQUEST).await else {
        return http::StatusCode::PAYLOAD_TOO_LARGE.into_response();
    };
    let Ok(message) = serde_json::from_slice::<Value>(&body) else {
        return axum::Json(failure(&Value::Null, -32700, "Invalid JSON")).into_response();
    };
    let Some(method) = message["method"].as_str() else {
        return axum::Json(failure(&message["id"], -32600, "Invalid request")).into_response();
    };
    // A notification has no id and takes no answer.
    let Some(id) = message.get("id").filter(|id| !id.is_null()) else {
        return http::StatusCode::ACCEPTED.into_response();
    };
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": message["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "tilde", "version": env!("CARGO_PKG_VERSION")},
        })),
        "ping" => Ok(json!({})),
        "tools/list" => list(chat, caller).await,
        "tools/call" => call(chat, caller, &message["params"]).await,
        _ => Err((-32601, "Method not found".to_owned())),
    };
    axum::Json(match result {
        Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
        Err((code, text)) => failure(id, code, &text),
    })
    .into_response()
}

async fn entries(chat: &Chat, caller: &Caller) -> Result<Vec<crate::tools::Entry>, (i64, String)> {
    if !caller.harness.features().tools {
        return Err((-32601, "This agent's harness does not support tools".into()));
    }
    let tools = chat
        .tools
        .as_ref()
        .ok_or((-32603, "Tools are not configured".to_owned()))?;
    let mut entries = tools.load(Owner::Agent(caller.agent)).await.map_err(|e| {
        (
            -32603,
            e.message.unwrap_or_else(|| "Tools are unavailable".into()),
        )
    })?;
    entries.retain(|(_, callee, _)| !matches!(callee, Callee::Sandbox(_)));
    Ok(entries)
}

async fn list(chat: &Chat, caller: &Caller) -> Result<Value, (i64, String)> {
    let tools: Vec<Value> = entries(chat, caller)
        .await?
        .into_iter()
        .map(|(definition, _, _)| {
            let schema = |text: &str| serde_json::from_str::<Value>(text).ok();
            let mut tool = json!({
                "name": name(&definition.name),
                "description": definition.description,
                "inputSchema": schema(&definition.input_schema_json)
                    .unwrap_or_else(|| json!({"type":"object"})),
            });
            if let Some(output) = schema(&definition.output_schema_json) {
                tool["outputSchema"] = output;
            }
            if let Some(hints) = definition.annotations.as_option() {
                tool["annotations"] = json!({
                    "readOnlyHint": hints.read_only,
                    "destructiveHint": hints.destructive,
                    "idempotentHint": hints.idempotent,
                    "openWorldHint": hints.open_world,
                });
            }
            tool
        })
        .collect();
    Ok(json!({"tools": tools}))
}

/// A tool's own failure is its result (`isError`), as MCP has it, so the model reads why; a
/// tool the agent does not have is a protocol error.
async fn call(chat: &Chat, caller: &Caller, params: &Value) -> Result<Value, (i64, String)> {
    let wanted = params["name"].as_str().unwrap_or_default();
    let (definition, callee, _) = entries(chat, caller)
        .await?
        .into_iter()
        .find(|(d, _, _)| name(&d.name) == wanted)
        .ok_or((-32602, format!("Unknown tool: {wanted}")))?;
    let input = match &params["arguments"] {
        Value::Null => json!({}),
        arguments => arguments.clone(),
    };
    let run = async {
        crate::tools::validate(&definition, &input)?;
        // `entries` found the tools, so they are configured.
        let tools = chat.tools.as_ref().expect("tools");
        tools
            .run(
                callee,
                crate::tools::Caller {
                    call_id: uuid::Uuid::new_v4(),
                    agent: Some(caller.agent),
                    thread: None,
                    run: None,
                    sandbox: None,
                },
                input.clone(),
            )
            .await
    };
    Ok(match run.await {
        Ok(output) => {
            let mut result =
                json!({"content":[{"type":"text","text":output.to_string()}],"isError":false});
            if output.is_object() {
                result["structuredContent"] = output;
            }
            result
        }
        Err(error) => {
            let text = error.message.unwrap_or_else(|| "The tool failed".into());
            json!({"content":[{"type":"text","text":text}],"isError":true})
        }
    })
}
