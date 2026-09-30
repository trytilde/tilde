//! Table-driven REST tools. Most managed providers map one tool to one HTTP call: `{name}`
//! placeholders in the path and the listed query fields come from the input, and the rest of the
//! input is the JSON body. Providers keep their own schemas and authentication; this module owns
//! request building and the upstream response contract.
use crate::chat::{providers::Access, tools::ToolResult};
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use serde_json::{Map, Value, json};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}
/// One tool that is one HTTP call.
pub struct Rest {
    pub name: &'static str,
    pub summary: &'static str,
    pub description: &'static str,
    pub verb: Verb,
    /// Relative to the provider's base URL, e.g. `/repos/{owner}/{repo}/issues`. A `{name*}`
    /// placeholder spans segments (a file path or `heads/main`): its `/` are kept.
    pub path: &'static str,
    /// Input fields sent as query parameters. For GET and DELETE every field that is not a path
    /// placeholder is a query parameter, so this only matters for calls with a body.
    pub query: &'static [&'static str],
}

/// A tool definition with the catalog's conventions: object schemas, provider id and hints.
pub fn definition(
    provider: &str,
    name: &str,
    summary: &str,
    description: &str,
    input_schema: Value,
    hints: Hints,
) -> types::ToolDefinition {
    types::ToolDefinition {
        name: name.into(),
        provider_id: provider.into(),
        description: description.into(),
        summary: summary.into(),
        input_schema_json: input_schema.to_string(),
        output_schema_json: json!({"type":"object"}).to_string(),
        annotations: types::ToolAnnotations {
            read_only: hints.read_only,
            destructive: hints.destructive,
            idempotent: hints.read_only,
            open_world: true,
            ..Default::default()
        }
        .into(),
        ..Default::default()
    }
}
#[derive(Clone, Copy, Default)]
pub struct Hints {
    pub read_only: bool,
    pub destructive: bool,
}
impl Verb {
    /// Reads are read-only and deletes destructive; writes are neither.
    pub fn hints(self) -> Hints {
        Hints {
            read_only: self == Verb::Get,
            destructive: self == Verb::Delete,
        }
    }
}

/// Build the request for `spec` against `base` (overridable through the `endpoint` key, as every
/// managed provider's base URL is, so tests can point it at a fake). Authentication is the
/// caller's: add headers to the returned builder.
pub fn request(
    access: &Access,
    endpoint: &str,
    base: &str,
    spec: &Rest,
    input: Value,
) -> ToolResult<reqwest::RequestBuilder> {
    let mut fields = match input {
        Value::Object(fields) => fields,
        Value::Null => Map::new(),
        _ => return Err(ConnectError::invalid_argument("Input must be an object")),
    };
    let mut path = String::new();
    let mut rest = spec.path;
    while let Some(start) = rest.find('{') {
        let end = rest[start..]
            .find('}')
            .map(|end| start + end)
            .ok_or_else(|| ConnectError::internal("Invalid tool path"))?;
        path.push_str(&rest[..start]);
        let (name, spans) = match rest[start + 1..end].strip_suffix('*') {
            Some(name) => (name, true),
            None => (&rest[start + 1..end], false),
        };
        let value = match fields.remove(name) {
            Some(Value::String(value)) if !value.is_empty() => value,
            Some(Value::Number(value)) => value.to_string(),
            _ => {
                return Err(ConnectError::invalid_argument(format!(
                    "`{name}` is required"
                )));
            }
        };
        let segments: Vec<&str> = if spans {
            value.split('/').collect()
        } else {
            vec![&value]
        };
        // Dot segments would be resolved away and escape the tool's endpoint.
        if segments
            .iter()
            .any(|s| s.is_empty() || *s == "." || *s == "..")
        {
            return Err(ConnectError::invalid_argument(format!(
                "`{name}` is not a valid path"
            )));
        }
        let encoded: Vec<String> = segments
            .into_iter()
            .map(|s| url::form_urlencoded::byte_serialize(s.as_bytes()).collect())
            .collect();
        path.push_str(&encoded.join("/"));
        rest = &rest[end + 1..];
    }
    path.push_str(rest);
    let mut url = url::Url::parse(
        access
            .endpoints
            .0
            .get(endpoint)
            .map(String::as_str)
            .unwrap_or(base),
    )
    .map_err(|_| ConnectError::internal("Invalid provider endpoint"))?;
    url.set_path(&format!("{}{}", url.path().trim_end_matches('/'), path));
    let bodiless = matches!(spec.verb, Verb::Get | Verb::Delete);
    let query: Vec<(String, Value)> = if bodiless {
        std::mem::take(&mut fields).into_iter().collect()
    } else {
        spec.query
            .iter()
            .filter_map(|key| fields.remove(*key).map(|value| ((*key).to_owned(), value)))
            .collect()
    };
    {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in query {
            match value {
                Value::Null => {}
                Value::String(value) => {
                    pairs.append_pair(&key, &value);
                }
                Value::Array(items) => {
                    for item in items {
                        pairs.append_pair(&key, &scalar(item));
                    }
                }
                other => {
                    pairs.append_pair(&key, &scalar(other));
                }
            }
        }
    }
    if url.query() == Some("") {
        url.set_query(None);
    }
    let client = &access.http.client;
    let builder = match spec.verb {
        Verb::Get => client.get(url),
        Verb::Delete => client.delete(url),
        Verb::Post => client.post(url),
        Verb::Put => client.put(url),
        Verb::Patch => client.patch(url),
    };
    Ok(if bodiless {
        builder
    } else {
        builder.json(&Value::Object(fields))
    })
}
fn scalar(value: Value) -> String {
    match value {
        Value::String(value) => value,
        other => other.to_string(),
    }
}

/// Send and read the upstream response as the tool's output: `{status, data}`, where `data` is
/// the parsed JSON body (or its text, or null when empty). A non-2xx status is the tool's error
/// with the upstream message, which the agent needs to correct its call; bodies are capped.
pub async fn send(request: reqwest::RequestBuilder) -> ToolResult<Value> {
    let (status, bytes) = response(request).await?;
    output(status, &bytes)
}
/// `send`'s response handling, for providers that also read response headers.
pub async fn read(response: reqwest::Response) -> ToolResult<Value> {
    let (status, bytes) = body(response).await?;
    output(status, &bytes)
}
/// Send and read the capped body without judging the status, for callers that branch on it or
/// need the raw bytes.
pub async fn response(
    request: reqwest::RequestBuilder,
) -> ToolResult<(reqwest::StatusCode, Vec<u8>)> {
    body(
        request
            .send()
            .await
            .map_err(|_| ConnectError::unavailable("The provider could not be reached"))?,
    )
    .await
}
async fn body(mut response: reqwest::Response) -> ToolResult<(reqwest::StatusCode, Vec<u8>)> {
    let status = response.status();
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ConnectError::unavailable("The provider response was interrupted"))?
    {
        if bytes.len() + chunk.len() > crate::tools::MAX_OUTPUT {
            return Err(ConnectError::resource_exhausted(
                crate::tools::OUTPUT_TOO_LARGE,
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok((status, bytes))
}
/// The `send` contract over an already read response.
pub fn output(status: reqwest::StatusCode, bytes: &[u8]) -> ToolResult<Value> {
    let data = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(bytes).into_owned()))
    };
    if !status.is_success() {
        let detail = match &data {
            Value::String(text) => text.clone(),
            Value::Null => String::new(),
            other => other.to_string(),
        };
        return Err(ConnectError::unknown(format!(
            "The provider returned {}: {}",
            status.as_u16(),
            detail.chars().take(2048).collect::<String>()
        )));
    }
    Ok(json!({"status": status.as_u16(), "data": data}))
}
