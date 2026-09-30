//! Google Drive: search, read, download/export, upload, share, move, copy and trash files.
use super::{Api, DELETE, READ, WRITE, call, optional, string_list, text, tool};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{categories::CATEGORY_FILE_STORAGE, model};
use crate::proto::tilde::types::v1 as types;
use crate::tools::providers::{ToolProvider, rest};
use base64::Engine;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use reqwest::Method;
use serde_json::{Value, json};

const ID: &str = "google_drive";

pub fn definition() -> model::Provider {
    super::definition(
        ID,
        "Google Drive",
        "Google Drive integration for searching, uploading, downloading, and managing files and folders.",
        CATEGORY_FILE_STORAGE,
        &["drive", "drive.file", "drive.readonly"],
    )
}

pub struct GoogleDrive;

impl ToolProvider for GoogleDrive {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        let file_id = json!({"type":"string","description":"The ID of the file"});
        let parent = json!({"type":"string","description":"Optional parent folder ID"});
        vec![
            tool(
                ID,
                "google_drive_search_files",
                "Search for files and folders using Drive query syntax",
                "Search for files and folders in Google Drive using query syntax. Example: \"name contains 'report'\".",
                READ,
                json!({
                    "query":{"type":"string","description":"Drive search query, e.g. \"name contains 'report'\""},
                    "max_results":{"type":"integer","minimum":0,"description":"Maximum number of results to return (default 20)"},
                    "page_token":{"type":"string","description":"Page token for pagination"}
                }),
                &["query"],
            ),
            tool(
                ID,
                "google_drive_get_file_metadata",
                "Get metadata for a file",
                "Get detailed metadata for a file in Google Drive.",
                READ,
                json!({"file_id":file_id}),
                &["file_id"],
            ),
            tool(
                ID,
                "google_drive_download_file",
                "Download a file. For Google Workspace files, exports to specified format.",
                "Download a file from Google Drive. For Google Workspace files (Docs, Sheets, Slides), exports to the specified format.",
                READ,
                json!({
                    "file_id":{"type":"string","description":"The ID of the file to download"},
                    "export_format":{"type":"string","description":"Export format for Google Workspace files (e.g. \"pdf\", \"text/plain\", \"text/csv\", \"application/vnd.openxmlformats-officedocument.wordprocessingml.document\")"}
                }),
                &["file_id"],
            ),
            tool(
                ID,
                "google_drive_upload_file",
                "Upload a file to Drive",
                "Upload a file to Google Drive. Content must be base64-encoded. Uses multipart upload.",
                WRITE,
                json!({
                    "name":{"type":"string","description":"File name"},
                    "content":{"type":"string","description":"Base64-encoded file content"},
                    "mime_type":{"type":"string","description":"MIME type of the file (e.g. \"application/pdf\")"},
                    "parent_folder_id":parent
                }),
                &["name", "content", "mime_type"],
            ),
            tool(
                ID,
                "google_drive_create_folder",
                "Create a new folder",
                "Create a new folder in Google Drive.",
                WRITE,
                json!({"name":{"type":"string","description":"Folder name"},"parent_folder_id":parent}),
                &["name"],
            ),
            tool(
                ID,
                "google_drive_share_file",
                "Share a file with users or make it public",
                "Share a file or folder in Google Drive with specific users or make it publicly accessible.",
                WRITE,
                json!({
                    "file_id":{"type":"string","description":"The ID of the file to share"},
                    "email":{"type":"string","description":"Email address of the user or group (required when share_type is \"user\", \"group\", or \"domain\")"},
                    "role":{"type":"string","description":"Permission role: \"reader\", \"writer\", or \"commenter\""},
                    "share_type":{"type":"string","description":"Permission type: \"user\", \"group\", \"domain\", or \"anyone\" (default \"user\")"}
                }),
                &["file_id", "role"],
            ),
            tool(
                ID,
                "google_drive_move_file",
                "Move a file to a different folder",
                "Move a file to a different folder in Google Drive.",
                WRITE,
                json!({
                    "file_id":{"type":"string","description":"The ID of the file to move"},
                    "destination_folder_id":{"type":"string","description":"The ID of the destination folder"}
                }),
                &["file_id", "destination_folder_id"],
            ),
            tool(
                ID,
                "google_drive_copy_file",
                "Create a copy of a file",
                "Create a copy of a file in Google Drive.",
                WRITE,
                json!({
                    "file_id":{"type":"string","description":"The ID of the file to copy"},
                    "name":{"type":"string","description":"Optional new name for the copy"},
                    "parent_folder_id":{"type":"string","description":"Optional parent folder ID for the copy"}
                }),
                &["file_id"],
            ),
            tool(
                ID,
                "google_drive_delete_file",
                "Move a file to trash",
                "Move a file to the trash in Google Drive (does not permanently delete).",
                DELETE,
                json!({"file_id":{"type":"string","description":"The ID of the file to move to trash"}}),
                &["file_id"],
            ),
            tool(
                ID,
                "google_drive_create_file_from_text",
                "Create a new file from text content",
                "Create a new file in Google Drive from plain text content.",
                WRITE,
                json!({
                    "name":{"type":"string","description":"File name"},
                    "content":{"type":"string","description":"Plain text content"},
                    "mime_type":{"type":"string","description":"MIME type (default \"text/plain\")"},
                    "parent_folder_id":parent
                }),
                &["name", "content"],
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
            let api = drive(access);
            let file = ["files", text(&input, "file_id")];
            match name {
                "google_drive_search_files" => {
                    let mut query = vec![
                        ("q", text(&input, "query").to_owned()),
                        (
                            "pageSize",
                            input["max_results"].as_u64().unwrap_or(20).to_string(),
                        ),
                        (
                            "fields",
                            "nextPageToken,files(id,name,mimeType,modifiedTime,size,owners,shared,parents)".into(),
                        ),
                        ("orderBy", "modifiedTime desc".into()),
                    ];
                    if let Some(token) = optional(&input, "page_token") {
                        query.push(("pageToken", token.to_owned()));
                    }
                    let found = call(api.get(&["files"])?.query(&query)).await?;
                    let files: Vec<Value> = found["files"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|file| {
                            json!({
                                "id": file["id"].as_str().unwrap_or_default(),
                                "name": file["name"].as_str().unwrap_or_default(),
                                "mime_type": file["mimeType"].as_str(),
                                "size": file["size"].as_str(),
                                "modified_time": file["modifiedTime"].as_str(),
                                "owners": owners(file),
                                "shared": file["shared"].as_bool(),
                                "parents": string_list(&file["parents"])
                            })
                        })
                        .collect();
                    Ok(json!({"files": files, "next_page_token": found["nextPageToken"].as_str()}))
                }
                "google_drive_get_file_metadata" => {
                    let body = call(api.get(&file)?.query(&[("fields", "*")])).await?;
                    Ok(json!({
                        "id": body["id"].as_str().unwrap_or_default(),
                        "name": body["name"].as_str().unwrap_or_default(),
                        "mime_type": body["mimeType"].as_str(),
                        "size": body["size"].as_str().and_then(|size| size.parse::<u64>().ok()),
                        "modified_time": body["modifiedTime"].as_str(),
                        "created_time": body["createdTime"].as_str(),
                        "owners": owners(&body),
                        "shared": body["shared"].as_bool().unwrap_or(false),
                        "web_view_link": body["webViewLink"].as_str(),
                        "parents": string_list(&body["parents"])
                    }))
                }
                "google_drive_download_file" => download(&api, &input).await,
                "google_drive_upload_file" => {
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(text(&input, "content"))
                        .map_err(|error| {
                            ConnectError::invalid_argument(format!(
                                "Invalid base64 content: {error}"
                            ))
                        })?;
                    upload(access, &input, text(&input, "mime_type"), bytes).await
                }
                "google_drive_create_file_from_text" => {
                    let mime = optional(&input, "mime_type").unwrap_or("text/plain");
                    upload(access, &input, mime, text(&input, "content").into()).await
                }
                "google_drive_create_folder" => {
                    let mut body = json!({
                        "name": text(&input, "name"),
                        "mimeType": "application/vnd.google-apps.folder"
                    });
                    if let Some(parent) = optional(&input, "parent_folder_id") {
                        body["parents"] = json!([parent]);
                    }
                    let folder = call(
                        api.post(&["files"])?
                            .query(&[("fields", "id,name")])
                            .json(&body),
                    )
                    .await?;
                    Ok(json!({
                        "folder_id": folder["id"].as_str().unwrap_or_default(),
                        "name": folder["name"].as_str().unwrap_or_default()
                    }))
                }
                "google_drive_share_file" => {
                    let mut body = json!({
                        "role": text(&input, "role"),
                        "type": optional(&input, "share_type").unwrap_or("user")
                    });
                    if let Some(email) = optional(&input, "email") {
                        body["emailAddress"] = json!(email);
                    }
                    let permission = call(
                        api.post(&["files", text(&input, "file_id"), "permissions"])?
                            .json(&body),
                    )
                    .await?;
                    Ok(json!({
                        "permission_id": permission["id"].as_str().unwrap_or_default(),
                        "success": true
                    }))
                }
                "google_drive_move_file" => {
                    let current = call(api.get(&file)?.query(&[("fields", "parents")])).await?;
                    call(api.request(Method::PATCH, &file)?.query(&[
                        ("addParents", text(&input, "destination_folder_id")),
                        ("removeParents", &string_list(&current["parents"]).join(",")),
                    ]))
                    .await?;
                    Ok(json!({"success": true}))
                }
                "google_drive_copy_file" => {
                    let mut body = json!({});
                    if let Some(name) = optional(&input, "name") {
                        body["name"] = json!(name);
                    }
                    if let Some(parent) = optional(&input, "parent_folder_id") {
                        body["parents"] = json!([parent]);
                    }
                    let copy = call(
                        api.post(&["files", text(&input, "file_id"), "copy"])?
                            .query(&[("fields", "id,name")])
                            .json(&body),
                    )
                    .await?;
                    Ok(json!({
                        "file_id": copy["id"].as_str().unwrap_or_default(),
                        "name": copy["name"].as_str().unwrap_or_default()
                    }))
                }
                "google_drive_delete_file" => {
                    call(
                        api.request(Method::PATCH, &file)?
                            .json(&json!({"trashed": true})),
                    )
                    .await?;
                    Ok(json!({"success": true}))
                }
                _ => Err(ConnectError::not_found("Unsupported Google Drive tool")),
            }
        })
    }
}

pub(super) fn drive(access: &Access) -> Api<'_> {
    Api::new(
        access,
        "google_drive_api",
        "https://www.googleapis.com/drive/v3",
    )
}

fn owners(file: &Value) -> Vec<&str> {
    file["owners"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|owner| owner["emailAddress"].as_str())
        .collect()
}

/// Exports Google Workspace files when `export_format` is set, otherwise downloads the content.
/// Text-like content is returned as text, anything else base64-encoded.
async fn download(api: &Api<'_>, input: &Value) -> ToolResult<Value> {
    let id = text(input, "file_id");
    let meta = call(
        api.get(&["files", id])?
            .query(&[("fields", "name,mimeType")]),
    )
    .await?;
    let (request, mime) = match optional(input, "export_format") {
        Some(format) => (
            api.get(&["files", id, "export"])?
                .query(&[("mimeType", format)]),
            format,
        ),
        None => (
            api.get(&["files", id])?.query(&[("alt", "media")]),
            meta["mimeType"]
                .as_str()
                .unwrap_or("application/octet-stream"),
        ),
    };
    let (status, bytes) = rest::response(request).await?;
    rest::output(status, &bytes)?;
    let textual = mime.starts_with("text/")
        || mime.contains("json")
        || mime.contains("xml")
        || mime.contains("csv");
    let content = match textual {
        true => String::from_utf8(bytes).unwrap_or_else(|error| {
            base64::engine::general_purpose::STANDARD.encode(error.as_bytes())
        }),
        false => base64::engine::general_purpose::STANDARD.encode(&bytes),
    };
    Ok(json!({
        "content": content,
        "mime_type": mime,
        "file_name": meta["name"].as_str().unwrap_or("unknown")
    }))
}

/// A multipart/related upload: JSON metadata then the content.
async fn upload(access: &Access, input: &Value, mime: &str, content: Vec<u8>) -> ToolResult<Value> {
    const BOUNDARY: &str = "tilde_mcp_boundary";
    let mut metadata = json!({"name": text(input, "name")});
    if let Some(parent) = optional(input, "parent_folder_id") {
        metadata["parents"] = json!([parent]);
    }
    let mut body = format!(
        "--{BOUNDARY}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{metadata}\r\n--{BOUNDARY}\r\nContent-Type: {mime}\r\n\r\n"
    )
    .into_bytes();
    body.extend(content);
    body.extend(format!("\r\n--{BOUNDARY}--").into_bytes());
    let upload = Api::new(
        access,
        "google_drive_upload",
        "https://www.googleapis.com/upload/drive/v3",
    );
    let file = call(
        upload
            .post(&["files"])?
            .query(&[
                ("uploadType", "multipart"),
                ("fields", "id,name,webViewLink"),
            ])
            .header(
                reqwest::header::CONTENT_TYPE,
                format!("multipart/related; boundary={BOUNDARY}"),
            )
            .body(body),
    )
    .await?;
    Ok(json!({
        "file_id": file["id"].as_str().unwrap_or_default(),
        "name": file["name"].as_str().unwrap_or_default(),
        "web_view_link": file["webViewLink"].as_str()
    }))
}
