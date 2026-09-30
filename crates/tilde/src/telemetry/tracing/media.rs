//! Multimodal payloads. A base64 data URI inside any string
//! attribute of a span is uploaded to the media bucket and replaced in place by a token,
//! `@@@tildeMedia:type=<mime>|id=<sha256>|source=base64_data_uri@@@`, so spans stay small
//! and searchable while the bytes are fetched on demand through a signed URL. Attributes that
//! are still large after that are stored whole in the bucket and truncated on the span, with
//! `<key>_ref` naming the full copy. This covers the projected `tilde.observation.*` fields
//! and the AI SDK or GenAI attributes they were copied from alike, so no full payload stays
//! inline anywhere in the span store or in what the viewer returns.
use crate::agent::avatar::ObjectStore;
use crate::error::Error;
use base64::Engine as _;
use opentelemetry_proto::tonic::{
    common::v1::{AnyValue, KeyValue, any_value::Value},
    trace::v1::ResourceSpans,
};
use sha2::{Digest, Sha256};

/// Inline text kept on the span; the rest lives in the bucket.
pub const INLINE_LIMIT: usize = 64 * 1024;
const MAX_MEDIA_BYTES: usize = 32 * 1024 * 1024;

fn agent_of(span: &opentelemetry_proto::tonic::trace::v1::Span) -> Option<String> {
    span.attributes
        .iter()
        .find(|a| a.key == "tilde.agent.id")
        .and_then(|a| a.value.as_ref())
        .and_then(|v| match &v.value {
            Some(Value::StringValue(s)) => Some(s.clone()),
            _ => None,
        })
}
fn set(attributes: &mut Vec<KeyValue>, key: &str, value: String) {
    attributes.retain(|a| a.key != key);
    attributes.push(KeyValue {
        key: key.into(),
        value: Some(AnyValue {
            value: Some(Value::StringValue(value)),
        }),
        ..Default::default()
    });
}
/// Every `data:<mime>;base64,<payload>` occurrence, as (start, end, mime, payload).
fn data_uris(text: &str) -> Vec<(usize, usize, String, String)> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = text[from..].find("data:") {
        let start = from + at;
        let rest = &text[start + 5..];
        let Some(semicolon) = rest.find(';') else {
            break;
        };
        let mime = &rest[..semicolon];
        let after = &rest[semicolon..];
        if mime.is_empty()
            || mime.len() > 100
            || !mime.contains('/')
            || !mime
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/+.-".contains(&b))
            || !after.starts_with(";base64,")
        {
            from = start + 5;
            continue;
        }
        let payload_start = start + 5 + semicolon + ";base64,".len();
        let payload_len = text[payload_start..]
            .bytes()
            .take_while(|b| b.is_ascii_alphanumeric() || *b == b'+' || *b == b'/' || *b == b'=')
            .count();
        if payload_len >= 16 {
            found.push((
                start,
                payload_start + payload_len,
                mime.to_owned(),
                text[payload_start..payload_start + payload_len].to_owned(),
            ));
        }
        from = payload_start + payload_len;
    }
    found
}
/// Upload media and oversized payloads of every span; attributes are rewritten in place.
pub async fn extract(store: &ObjectStore, resources: &mut [ResourceSpans]) -> Result<(), Error> {
    for span in resources
        .iter_mut()
        .flat_map(|r| &mut r.scope_spans)
        .flat_map(|s| &mut s.spans)
    {
        let Some(agent) = agent_of(span) else {
            continue;
        };
        // Every string attribute, not only the projected observation fields: the source a
        // field was copied from carries the same payload.
        let fields: Vec<(String, String)> = span
            .attributes
            .iter()
            .filter(|a| !a.key.ends_with("_ref"))
            .filter_map(|a| match a.value.as_ref()?.value.as_ref()? {
                Value::StringValue(text) if text.len() > INLINE_LIMIT || text.contains("data:") => {
                    Some((a.key.clone(), text.clone()))
                }
                _ => None,
            })
            .collect();
        for (field, text) in fields {
            let mut rewritten = String::with_capacity(text.len());
            let mut cursor = 0;
            for (start, end, mime, payload) in data_uris(&text) {
                let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&payload) else {
                    continue;
                };
                if bytes.len() > MAX_MEDIA_BYTES {
                    continue;
                }
                let id = hex::encode(Sha256::digest(&bytes));
                // Content-addressed: the same image twice is one object.
                store
                    .write(
                        reqwest::Method::PUT,
                        &format!("media/{agent}/{id}"),
                        &mime,
                        bytes,
                    )
                    .await?;
                rewritten.push_str(&text[cursor..start]);
                rewritten.push_str(&format!(
                    "@@@tildeMedia:type={mime}|id={id}|source=base64_data_uri@@@"
                ));
                cursor = end;
            }
            rewritten.push_str(&text[cursor..]);
            if rewritten.len() > INLINE_LIMIT {
                // Content-addressed too, so a projected copy and its source share one object.
                let id = hex::encode(Sha256::digest(rewritten.as_bytes()));
                let key = format!("payloads/{agent}/{id}");
                store
                    .write(
                        reqwest::Method::PUT,
                        &key,
                        "text/plain; charset=utf-8",
                        rewritten.clone().into_bytes(),
                    )
                    .await?;
                let mut cut = INLINE_LIMIT;
                while !rewritten.is_char_boundary(cut) {
                    cut -= 1;
                }
                rewritten.truncate(cut);
                set(&mut span.attributes, &format!("{field}_ref"), key);
            }
            if rewritten != text {
                set(&mut span.attributes, &field, rewritten);
            }
        }
    }
    Ok(())
}
/// Keys a viewer may resolve: only this agent's media and payloads.
pub fn owned_by(key: &str, agent: &str) -> bool {
    let hex = |s: &str| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit());
    match key.split('/').collect::<Vec<_>>().as_slice() {
        ["media" | "payloads", owner, id] => *owner == agent && hex(id),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn data_uris_are_found_and_bounded() {
        let text = r#"{"image":"data:image/png;base64,iVBORw0KGgoAAAANSUhEUg==","x":"data:;base64,AAAA","y":"data:text/plain;base64,short"}"#;
        let found = data_uris(text);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].2, "image/png");
        assert!(text[found[0].0..found[0].1].ends_with("=="));
        assert!(owned_by(&format!("media/a/{}", "0".repeat(64)), "a"));
        assert!(!owned_by(&format!("media/b/{}", "0".repeat(64)), "a"));
        assert!(!owned_by("media/a/../secret", "a"));
    }
}
