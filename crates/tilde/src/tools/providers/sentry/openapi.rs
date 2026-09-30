//! Sentry REST operations from the OpenAPI document: each input carries its path and query
//! parameters by name, an optional `regionUrl`, and the remaining fields as the request body.
use super::{CATALOG, Client, encode, invalid, official, rest, scalar, send};
use crate::chat::tools::ToolResult;
use reqwest::Method;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
pub(super) struct Operation {
    pub operation_id: String,
    pub method: String,
    pub path: String,
    pub summary: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Value,
    parameters: Vec<Parameter>,
    request_body: Option<Body>,
    deprecated: bool,
}
#[derive(Deserialize)]
struct Parameter {
    name: String,
    location: String,
    required: bool,
}
#[derive(Deserialize)]
struct Body {
    content_type: String,
    schema: Value,
}
impl Operation {
    pub fn hints(&self) -> rest::Hints {
        rest::Hints {
            read_only: self.method == "GET",
            destructive: self.method == "DELETE",
        }
    }
}

/// Operations offered as their own tools: not deprecated and not covered by an official tool.
pub(super) fn operations() -> impl Iterator<Item = &'static Operation> {
    CATALOG
        .openapi_operations
        .iter()
        .filter(|operation| !operation.deprecated && !official::covers(&operation.operation_id))
}
pub(super) fn generated(name: &str) -> Option<&'static Operation> {
    operations().find(|operation| snake_case(&operation.operation_id) == name)
}
/// Any catalog operation by its OpenAPI ID, for official tools and the generic dispatcher.
pub(super) fn operation(id: &str) -> ToolResult<&'static Operation> {
    CATALOG
        .openapi_operations
        .iter()
        .find(|operation| operation.operation_id == id)
        .ok_or_else(|| invalid(format!("Unknown Sentry operation {id}")))
}

pub(super) async fn invoke(
    client: &Client<'_>,
    operation: &Operation,
    input: Value,
) -> ToolResult<Value> {
    let Value::Object(mut fields) = input else {
        return Err(invalid("Sentry tool input must be an object"));
    };
    let region = fields.remove("regionUrl");
    let base = client.base(region.as_ref().and_then(Value::as_str))?;
    let mut path = operation.path.clone();
    let mut query = Vec::new();
    for parameter in &operation.parameters {
        let Some(value) = fields.remove(&parameter.name) else {
            if parameter.required {
                return Err(invalid(format!(
                    "Missing required parameter {}",
                    parameter.name
                )));
            }
            continue;
        };
        match parameter.location.as_str() {
            "path" => {
                path = path.replace(&format!("{{{}}}", parameter.name), &encode(&scalar(&value)))
            }
            "query" => query.push((parameter.name.clone(), value)),
            _ => {}
        }
    }
    if path.contains('{') {
        return Err(invalid(format!(
            "Not all path parameters were supplied for {}",
            operation.operation_id
        )));
    }
    let method = Method::from_bytes(operation.method.as_bytes())
        .map_err(|_| invalid("Invalid Sentry operation method"))?;
    let mut request = client.request(method, &base, &path, &query)?;
    if let Some(body) = &operation.request_body {
        let value = if body.schema["type"] == "object" {
            Value::Object(fields)
        } else {
            fields.remove("body").unwrap_or(Value::Null)
        };
        request = match body.content_type.as_str() {
            "application/json" => request.json(&value),
            "multipart/form-data" => {
                let Value::Object(parts) = value else {
                    return Err(invalid("A multipart Sentry body must be an object"));
                };
                // Only release file uploads are multipart; `file` is sent as the file part.
                let boundary = format!("tilde-{}", uuid::Uuid::new_v4().simple());
                let mut encoded = String::new();
                for (name, value) in parts {
                    let filename = if name == "file" {
                        "; filename=\"file\""
                    } else {
                        ""
                    };
                    encoded.push_str(&format!(
                        "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"{filename}\r\n\r\n{}\r\n",
                        scalar(&value)
                    ));
                }
                encoded.push_str(&format!("--{boundary}--\r\n"));
                request
                    .header(
                        reqwest::header::CONTENT_TYPE,
                        format!("multipart/form-data; boundary={boundary}"),
                    )
                    .body(encoded)
            }
            other => return Err(invalid(format!("Unsupported Sentry request body {other}"))),
        };
    }
    send(request).await
}

pub(super) fn snake_case(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 8);
    for (index, character) in value.chars().enumerate() {
        if character.is_ascii_uppercase() {
            if index > 0 {
                output.push('_');
            }
            output.push(character.to_ascii_lowercase());
        } else if character.is_ascii_alphanumeric() {
            output.push(character.to_ascii_lowercase());
        } else if !output.ends_with('_') {
            output.push('_');
        }
    }
    output.trim_matches('_').to_owned()
}
