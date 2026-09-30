//! Google Docs: create, read as plain text, edit through `:batchUpdate`, and copy (via Drive).
use super::{Api, READ, WRITE, call, optional, text, tool};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{categories::CATEGORY_DOCUMENTS, model};
use crate::proto::tilde::types::v1 as types;
use crate::tools::providers::ToolProvider;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use serde_json::{Value, json};

const ID: &str = "google_docs";

pub fn definition() -> model::Provider {
    super::definition(
        ID,
        "Google Docs",
        "Google Docs integration for creating, reading, and editing documents.",
        CATEGORY_DOCUMENTS,
        &["documents", "documents.readonly", "drive.file"],
    )
}

pub struct GoogleDocs;

impl ToolProvider for GoogleDocs {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        let s = json!({"type":"string"});
        vec![
            tool(
                ID,
                "google_docs_create_document",
                "Create a new Google Doc with optional initial content",
                "Create a new Google Doc with optional initial content",
                WRITE,
                json!({"title":s,"content":s}),
                &["title"],
            ),
            tool(
                ID,
                "google_docs_get_document",
                "Retrieve a document's content as plain text",
                "Retrieve a document's content as plain text",
                READ,
                json!({"document_id":s}),
                &["document_id"],
            ),
            tool(
                ID,
                "google_docs_append_text",
                "Append text to the end of a document",
                "Append text to the end of a document",
                WRITE,
                json!({"document_id":s,"content":s}),
                &["document_id", "content"],
            ),
            tool(
                ID,
                "google_docs_insert_text",
                "Insert text at a specific position in a document",
                "Insert text at a specific position in a document",
                WRITE,
                json!({"document_id":s,"content":s,"index":{"type":"integer","minimum":0}}),
                &["document_id", "content", "index"],
            ),
            tool(
                ID,
                "google_docs_replace_text",
                "Find and replace text within a document",
                "Find and replace text within a document",
                WRITE,
                json!({"document_id":s,"find":s,"replace":s,"match_case":{"type":"boolean"}}),
                &["document_id", "find", "replace"],
            ),
            tool(
                ID,
                "google_docs_copy_document",
                "Create a copy of an existing document",
                "Create a copy of an existing document",
                WRITE,
                json!({"document_id":s,"title":s}),
                &["document_id"],
            ),
        ]
    }
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        _call_id: uuid::Uuid,
        name: &'a str,
        input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            let api = Api::new(access, "google_docs_api", "https://docs.googleapis.com/v1");
            let id = text(&input, "document_id");
            match name {
                "google_docs_create_document" => {
                    let title = text(&input, "title");
                    let created =
                        call(api.post(&["documents"])?.json(&json!({"title": title}))).await?;
                    let id = created["documentId"]
                        .as_str()
                        .ok_or_else(|| ConnectError::unknown("No documentId in response"))?;
                    if let Some(content) = optional(&input, "content") {
                        batch(
                            &api,
                            id,
                            json!({"insertText": {"text": content, "location": {"index": 1}}}),
                        )
                        .await?;
                    }
                    Ok(json!({
                        "document_id": id,
                        "title": created["title"].as_str().unwrap_or(title),
                        "document_link": link(id)
                    }))
                }
                "google_docs_get_document" => {
                    let document = call(api.get(&["documents", id])?).await?;
                    let content: String = document["body"]["content"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .flat_map(|element| {
                            element["paragraph"]["elements"]
                                .as_array()
                                .into_iter()
                                .flatten()
                        })
                        .filter_map(|element| element["textRun"]["content"].as_str())
                        .collect();
                    Ok(json!({
                        "document_id": document["documentId"].as_str().unwrap_or(id),
                        "title": document["title"].as_str().unwrap_or_default(),
                        "content": content,
                        "revision_id": document["revisionId"].as_str()
                    }))
                }
                "google_docs_append_text" => {
                    batch(
                        &api,
                        id,
                        json!({"insertText": {"text": text(&input, "content"), "endOfSegmentLocation": {"segmentId": ""}}}),
                    )
                    .await?;
                    Ok(json!({"success": true}))
                }
                "google_docs_insert_text" => {
                    batch(
                        &api,
                        id,
                        json!({"insertText": {"text": text(&input, "content"), "location": {"index": input["index"]}}}),
                    )
                    .await?;
                    Ok(json!({"success": true}))
                }
                "google_docs_replace_text" => {
                    let replies = batch(
                        &api,
                        id,
                        json!({"replaceAllText": {
                            "containsText": {
                                "text": text(&input, "find"),
                                "matchCase": input["match_case"].as_bool().unwrap_or(true)
                            },
                            "replaceText": text(&input, "replace")
                        }}),
                    )
                    .await?;
                    Ok(json!({
                        "occurrences_changed": replies["replies"][0]["replaceAllText"]["occurrencesChanged"].as_u64().unwrap_or(0)
                    }))
                }
                "google_docs_copy_document" => {
                    let mut body = json!({});
                    if let Some(title) = optional(&input, "title") {
                        body["name"] = json!(title);
                    }
                    let copy = call(
                        super::drive::drive(access)
                            .post(&["files", id, "copy"])?
                            .json(&body),
                    )
                    .await?;
                    let id = copy["id"]
                        .as_str()
                        .ok_or_else(|| ConnectError::unknown("No id in copy response"))?;
                    Ok(json!({
                        "document_id": id,
                        "title": copy["name"].as_str().unwrap_or_default(),
                        "document_link": link(id)
                    }))
                }
                _ => Err(ConnectError::not_found("Unsupported Google Docs tool")),
            }
        })
    }
}

async fn batch(api: &Api<'_>, id: &str, request: Value) -> ToolResult<Value> {
    call(
        api.post(&["documents", &format!("{id}:batchUpdate")])?
            .json(&json!({"requests": [request]})),
    )
    .await
}
fn link(id: &str) -> String {
    format!("https://docs.google.com/document/d/{id}/edit")
}
