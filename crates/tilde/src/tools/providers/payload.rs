//! Payload CMS collections and globals over the instance's generated REST API, authenticated
//! with an API key of an auth-enabled collection (`Authorization: <collection> API-Key <key>`).
use super::{
    ToolProvider,
    rest::{self, Verb},
};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{
    categories::CATEGORY_DOCUMENTS,
    model::{self, optional},
};
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use serde_json::{Map, Value, json};

pub fn definition() -> model::Provider {
    model::Provider {
        account_name_label: Some("Payload instance name".into()),
        icon_url: Some("/provider-icons/payload.svg".into()),
        instructions: Some(
            "Connect a Payload CMS instance so agents can read and edit its documents. Enable API keys (auth.useAPIKey) on an auth collection, generate a key for a user of it, and give the REST API URL, e.g. https://cms.example.com/api."
                .into(),
        ),
        id: "payload".into(),
        name: "Payload CMS".into(),
        kind: model::ProviderKind::BuiltIn,
        categories: vec![CATEGORY_DOCUMENTS.into()],
        connection_types: vec![model::ConnectionType {
            mcp: None,
            id: "api".into(),
            name: "Payload API key".into(),
            credential_source: model::CredentialSource::Static {
                schema: json!({"type":"object","properties":{
                    "api_base_url":{"type":"string","title":"REST API URL","format":"uri","pattern":"^https?://","description":"The https URL of the Payload REST API."},
                    "auth_collection_slug":{"type":"string","title":"Auth collection","default":"users","pattern":"^[A-Za-z0-9_-]+$"},
                    "api_key":{"type":"string","title":"API key","minLength":1,"writeOnly":true}
                },"required":["api_base_url","api_key"],"additionalProperties":false}),
            },
            capabilities: vec![model::Capability::Tool],
        }],
    }
}

pub struct Payload;

use Verb::{Delete, Get, Patch, Post};
const TOOLS: &[(&str, &str, &str, Verb)] = &[
    (
        "find_documents",
        "Find documents",
        "Find documents in a collection, or one by `id`, with pagination, sorting and a `where` filter.",
        Get,
    ),
    (
        "count_documents",
        "Count documents",
        "Count the documents in a collection, optionally matching a `where` filter.",
        Get,
    ),
    (
        "create_document",
        "Create document",
        "Create a document in a collection from `data`.",
        Post,
    ),
    (
        "update_document",
        "Update document",
        "Update one document by `id`, or every document matching `where`, with the fields in `data`.",
        Patch,
    ),
    (
        "delete_documents",
        "Delete documents",
        "Delete one document by `id`, or every document matching `where`.",
        Delete,
    ),
    (
        "find_global",
        "Find global",
        "Fetch a global singleton by its slug.",
        Get,
    ),
    (
        "update_global",
        "Update global",
        "Update a global singleton with the fields in `data`.",
        Post,
    ),
];

fn slug() -> Value {
    json!({"type":"string","pattern":"^[A-Za-z0-9_-]+$"})
}
fn schema(name: &str) -> Value {
    let mut properties = json!({
        "depth":{"type":"integer","minimum":0,"maximum":10,"description":"How deep to populate relationships."},
        "draft":{"type":"boolean"},
        "locale":{"type":"string"},
        "fallback_locale":{"type":"string"},
        "populate":{"type":"object"},
        "select":{"type":"object","description":"Fields to return, e.g. {\"title\":true}."}
    });
    let global = name.ends_with("_global");
    let mut required = vec![if global {
        "global_slug"
    } else {
        "collection_slug"
    }];
    properties[required[0]] = slug();
    if !global {
        let collection = json!({
            "id":{"type":["string","integer"],"minLength":1},
            "where":{"type":"object","description":"Payload query, e.g. {\"title\":{\"equals\":\"Hello\"}} or {\"or\":[...]}."},
            "joins":{"type":"object"},
            "limit":{"type":"integer","minimum":0},
            "page":{"type":"integer","minimum":1},
            "pagination":{"type":"boolean"},
            "sort":{"type":"string","description":"Field name; prefix with - for descending."},
            "trash":{"type":"boolean"}
        });
        properties
            .as_object_mut()
            .expect("object")
            .extend(collection.as_object().cloned().unwrap_or_default());
    }
    match name {
        "count_documents" => {
            properties = json!({"collection_slug":slug(),"where":properties["where"].clone()});
        }
        "create_document" | "update_document" | "update_global" => {
            properties["data"] = json!({"type":"object"});
            required.push("data");
        }
        _ => {}
    }
    if name == "create_document"
        && let Some(properties) = properties.as_object_mut()
    {
        properties.remove("id");
        properties.remove("where");
    }
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}

impl ToolProvider for Payload {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        TOOLS
            .iter()
            .map(|(name, summary, description, verb)| {
                rest::definition(
                    "payload",
                    name,
                    summary,
                    description,
                    schema(name),
                    verb.hints(),
                )
            })
            .collect()
    }
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        _call_id: uuid::Uuid,
        name: &'a str,
        input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            let (_, _, _, verb) = TOOLS
                .iter()
                .find(|(tool, ..)| *tool == name)
                .ok_or_else(|| ConnectError::not_found("Unsupported Payload tool"))?;
            let mut input = match input {
                Value::Object(input) => input,
                _ => Map::new(),
            };
            if matches!(name, "update_document" | "delete_documents")
                && !input.contains_key("id")
                && !input.contains_key("where")
            {
                return Err(ConnectError::invalid_argument("Give id or where"));
            }
            let mut path = vec![];
            if let Some(global) = input.remove("global_slug") {
                path.push("globals".to_owned());
                path.push(segment(global)?);
            }
            if let Some(collection) = input.remove("collection_slug") {
                path.push(segment(collection)?);
            }
            if let Some(id) = input.remove("id") {
                path.push(segment(id)?);
            }
            if name == "count_documents" {
                path.push("count".into());
            }
            let body = input.remove("data");
            let mut url = base(access)?;
            url.path_segments_mut()
                .map_err(|_| ConnectError::failed_precondition("Invalid Payload REST API URL"))?
                .pop_if_empty()
                .extend(&path);
            {
                let mut pairs = url.query_pairs_mut();
                for (key, value) in input {
                    let key = if key == "fallback_locale" {
                        "fallbackLocale".to_owned()
                    } else {
                        key
                    };
                    query(&mut pairs, key, value);
                }
            }
            if url.query() == Some("") {
                url.set_query(None);
            }
            let client = if url.scheme() == "https" {
                crate::chat::providers::files::public_client(
                    &url,
                    std::time::Duration::from_secs(25),
                )
                .await?
            } else {
                access.http.client.clone()
            };
            let authorization = zeroize::Zeroizing::new(format!(
                "{} API-Key {}",
                optional(&access.values, "auth_collection_slug").unwrap_or("users"),
                access.secret("api_key")?
            ));
            let mut header = reqwest::header::HeaderValue::from_str(&authorization)
                .map_err(|_| ConnectError::failed_precondition("Invalid Payload credentials"))?;
            header.set_sensitive(true);
            let request = match verb {
                Get => client.get(url),
                Post => client.post(url),
                Patch => client.patch(url),
                _ => client.delete(url),
            }
            .header(reqwest::header::AUTHORIZATION, header)
            .header(reqwest::header::ACCEPT, "application/json");
            rest::send(match body {
                Some(body) => request.json(&body),
                None => request,
            })
            .await
        })
    }
}

/// The connection's REST API URL: https, or plain http to the one origin the installation allows
/// through the `payload_private_origin` endpoint (a local instance in development or tests), so
/// a connection cannot point the gateway at other private services.
fn base(access: &Access) -> ToolResult<url::Url> {
    let url = url::Url::parse(access.secret("api_base_url")?)
        .map_err(|_| ConnectError::failed_precondition("Invalid Payload REST API URL"))?;
    let allowed = access
        .endpoints
        .0
        .get("payload_private_origin")
        .map(String::as_str)
        == Some(url.origin().ascii_serialization().as_str());
    if url.scheme() == "https" || allowed {
        Ok(url)
    } else {
        Err(ConnectError::failed_precondition(
            "The Payload REST API URL must use https",
        ))
    }
}
fn segment(value: Value) -> ToolResult<String> {
    let value = match value {
        Value::String(value) => value,
        Value::Number(value) => value.to_string(),
        _ => String::new(),
    };
    if value.is_empty() || value == "." || value == ".." {
        return Err(ConnectError::invalid_argument("Invalid slug or id"));
    }
    Ok(value)
}
/// Payload parses nested query parameters in `qs` bracket syntax: `where[title][equals]=x`.
fn query(
    pairs: &mut url::form_urlencoded::Serializer<'_, url::UrlQuery<'_>>,
    key: String,
    value: Value,
) {
    match value {
        Value::Null => {}
        Value::Object(fields) => {
            for (field, value) in fields {
                query(pairs, format!("{key}[{field}]"), value);
            }
        }
        Value::Array(items) => {
            for (index, value) in items.into_iter().enumerate() {
                query(pairs, format!("{key}[{index}]"), value);
            }
        }
        Value::String(value) => {
            pairs.append_pair(&key, &value);
        }
        other => {
            pairs.append_pair(&key, &other.to_string());
        }
    }
}
