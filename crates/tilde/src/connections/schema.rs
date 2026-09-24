//! The supported flat JSON Schema subset is shared by validation and the standard credential form.
//! Schemas never resolve URLs/files. Complex or conditional forms belong to a custom source.
use super::{model::*, oauth::PrivateJson};
use crate::error::Error;
use secrecy::ExposeSecret;
use serde_json::{Value, json};

pub fn empty() -> Value {
    json!({"type":"object","properties":{},"additionalProperties":false})
}
/// Reject unsupported schema features rather than silently rendering an incomplete form.
pub fn validate_schema(schema: &Value) -> Result<(), Error> {
    let root = schema
        .as_object()
        .ok_or_else(|| invalid("Credential schema must be an object"))?;
    if root.keys().any(|key| {
        ![
            "$schema",
            "title",
            "description",
            "type",
            "properties",
            "required",
            "additionalProperties",
        ]
        .contains(&key.as_str())
    }) {
        return Err(invalid(
            "Unsupported credential schema keyword; use a custom source for complex forms",
        ));
    }
    if schema["type"] != "object" || schema["additionalProperties"] != false {
        return Err(invalid(
            "Credential schemas must declare an object with additionalProperties: false",
        ));
    }
    let properties = schema["properties"]
        .as_object()
        .ok_or_else(|| invalid("Credential properties are required"))?;
    if properties.len() > 100 || schema.to_string().len() > 65536 {
        return Err(invalid("Credential schema is too large"));
    }
    for (key, property) in properties {
        identifier(key)?;
        if key.starts_with('_') {
            return Err(invalid("Reserved credential key"));
        }
        let object = property
            .as_object()
            .ok_or_else(|| invalid("Credential property must be a schema object"))?;
        if object.keys().any(|key| {
            ![
                "type",
                "title",
                "description",
                "writeOnly",
                "enum",
                "default",
                "minLength",
                "maxLength",
                "pattern",
                "format",
                "minimum",
                "maximum",
                "contentMediaType",
            ]
            .contains(&key.as_str())
        }) {
            return Err(invalid("Unsupported credential property keyword"));
        }
        if !matches!(
            property["type"].as_str(),
            Some("string" | "boolean" | "integer" | "number")
        ) {
            return Err(invalid(
                "Credential properties support strings, booleans and numbers",
            ));
        }
        if let Some(format) = property.get("format").and_then(Value::as_str)
            && !["uri", "email"].contains(&format)
        {
            return Err(invalid("Unsupported credential format"));
        }
        if property
            .get("enum")
            .and_then(Value::as_array)
            .is_some_and(|items| items.len() > 100)
        {
            return Err(invalid("Too many credential choices"));
        }
        if let Some(choices) = property.get("enum").and_then(Value::as_array)
            && choices
                .iter()
                .any(|choice| match property["type"].as_str() {
                    Some("string") => !choice.is_string(),
                    Some("boolean") => !choice.is_boolean(),
                    Some("integer") => !(choice.is_i64() || choice.is_u64()),
                    Some("number") => !choice.is_number(),
                    _ => true,
                })
        {
            return Err(invalid(
                "Credential choices must match the declared primitive type",
            ));
        }
        if property["writeOnly"] == true
            && (property.get("default").is_some() || property.get("enum").is_some())
        {
            return Err(invalid(
                "Secret defaults or choices must not be stored in provider definitions",
            ));
        }
    }
    if let Some(required) = schema.get("required") {
        let keys = required
            .as_array()
            .ok_or_else(|| invalid("Required must be an array"))?;
        if keys
            .iter()
            .any(|key| key.as_str().is_none_or(|key| !properties.contains_key(key)))
        {
            return Err(invalid("Required keys must be declared properties"));
        }
    }
    jsonschema::options()
        .should_validate_formats(true)
        .build(schema)
        .map_err(|_| invalid("Invalid credential JSON Schema"))?;
    Ok(())
}
/// Values remain zeroizing strings in storage; validate their declared JSON types at this seam.
pub fn validate_values(schema: &Value, values: &Values) -> Result<(), Error> {
    validate_schema(schema)?;
    let mut data = PrivateJson(json!({}));
    for (key, value) in values {
        let text = value.expose_secret();
        if text.len() > 65536 {
            return Err(invalid("Credential value is too large"));
        }
        let property = &schema["properties"][key];
        let value = match property["type"].as_str() {
            Some("string") => Value::String(text.into()),
            Some("boolean") => match text {
                "true" => Value::Bool(true),
                "false" => Value::Bool(false),
                _ => return Err(invalid("Invalid boolean credential")),
            },
            Some("integer" | "number") => serde_json::from_str::<serde_json::Number>(text)
                .map(Value::Number)
                .map_err(|_| invalid("Invalid numeric credential"))?,
            _ => return Err(invalid("Unknown credential key")),
        };
        data.0[key] = value;
    }
    let validator = jsonschema::options()
        .should_validate_formats(true)
        .build(schema)
        .map_err(|_| invalid("Invalid credential JSON Schema"))?;
    if !validator.is_valid(&data.0) {
        return Err(invalid("Credentials do not match the declared schema"));
    }
    Ok(())
}
/// Protocol-defined OAuth inputs; providers supply endpoints and scopes, not custom form descriptors.
pub fn oauth_inputs(grant: OAuthGrant, config: &OAuth, additional: Option<&Value>) -> Value {
    let mut schema = empty();
    if grant == OAuthGrant::JwtBearer {
        schema["properties"] = json!({"issuer":{"type":"string","title":"Service account issuer","minLength":1},"private_key":{"type":"string","title":"Private key (PEM)","writeOnly":true,"contentMediaType":"application/x-pem-file","minLength":1},"subject":{"type":"string","title":"Subject (optional)"}});
        schema["required"] = json!(["issuer", "private_key"]);
    } else {
        schema["properties"] =
            json!({"client_id":{"type":"string","title":"Client ID","minLength":1}});
        schema["required"] = json!(["client_id"]);
        if config.client_auth != ClientAuth::None {
            schema["properties"]["client_secret"] =
                json!({"type":"string","title":"Client secret","writeOnly":true,"minLength":1});
            schema["required"]
                .as_array_mut()
                .expect("array")
                .push(json!("client_secret"));
        }
    }
    if let Some(additional) = additional {
        if let Some(properties) = additional["properties"].as_object() {
            for (key, value) in properties {
                schema["properties"][key] = value.clone();
            }
        }
        if let Some(required) = additional["required"].as_array() {
            schema["required"]
                .as_array_mut()
                .expect("array")
                .extend(required.iter().cloned());
        }
    }
    schema
}
