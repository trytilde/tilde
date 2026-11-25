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
/// A row of a provider's tool table: the call and its input fields.
pub struct Tool {
    pub rest: Rest,
    /// Space-separated input fields beside the path placeholders; `!` marks required ones.
    pub fields: &'static str,
}
pub const fn tool(
    name: &'static str,
    summary: &'static str,
    description: &'static str,
    verb: Verb,
    path: &'static str,
    fields: &'static str,
) -> Tool {
    Tool {
        rest: Rest {
            name,
            summary,
            description,
            verb,
            path,
            query: &[],
        },
        fields,
    }
}
impl Tool {
    /// The definition of a table tool: its path placeholders (required) and fields, each with
    /// the schema `property` gives its name. Other fields pass through to the call.
    pub fn definition(
        &self,
        provider: &str,
        property: impl Fn(&str) -> Value,
    ) -> types::ToolDefinition {
        let mut properties = Map::new();
        let mut required = vec![];
        let placeholders = self
            .rest
            .path
            .split('{')
            .skip(1)
            .filter_map(|part| part.split_once('}'))
            .map(|(name, _)| format!("!{}", name.trim_end_matches('*')));
        for field in placeholders.chain(self.fields.split_whitespace().map(str::to_owned)) {
            let name = field.trim_start_matches('!');
            if field.starts_with('!') {
                required.push(name.to_owned());
            }
            properties.insert(name.to_owned(), property(name));
        }
        definition(
            provider,
            self.rest.name,
            self.rest.summary,
            self.rest.description,
            json!({"type":"object","properties":properties,"required":required,"additionalProperties":true}),
            self.rest.verb.hints(),
        )
    }
}
/// Drops `field` from a definition's input: the provider fills it in from the connection.
pub fn hide(mut definition: types::ToolDefinition, field: &str) -> types::ToolDefinition {
    if let Ok(mut schema) = serde_json::from_str::<Value>(&definition.input_schema_json) {
        if let Some(properties) = schema["properties"].as_object_mut() {
            properties.remove(field);
        }
        if let Some(required) = schema["required"].as_array_mut() {
            required.retain(|name| name != field);
        }
        definition.input_schema_json = schema.to_string();
    }
    definition
}
/// The row named `name`.
pub fn find<'t>(tools: &'t [Tool], name: &str) -> ToolResult<&'t Tool> {
    tools
        .iter()
        .find(|tool| tool.rest.name == name)
        .ok_or_else(|| ConnectError::not_found("Unsupported tool"))
}
/// A base URL the connection configures (a self-managed instance) must be https, so the
/// credential never travels in the clear. Endpoint overrides (tests) replace it later.
pub fn https(url: &str) -> ToolResult<&str> {
    if url.starts_with("https://") {
        Ok(url.trim_end_matches('/'))
    } else {
        Err(ConnectError::failed_precondition(
            "The connection's URL must use https",
        ))
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
/// `value` without its null members and the objects that leaves empty, for request bodies
/// assembled from optional inputs.
pub fn without_nulls(value: Value) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, value)| (key, without_nulls(value)))
                .filter(|(_, value)| {
                    !value.is_null() && value.as_object().is_none_or(|o| !o.is_empty())
                })
                .collect(),
        ),
        other => other,
    }
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
