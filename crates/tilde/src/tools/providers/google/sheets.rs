//! Google Sheets: read, write, append, search and clear ranges; create spreadsheets and tabs;
//! format cells.
use super::{Api, DELETE, READ, WRITE, call, optional, text, tool};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{categories::CATEGORY_SPREADSHEETS, model};
use crate::proto::tilde::types::v1 as types;
use crate::tools::providers::ToolProvider;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use reqwest::Method;
use serde_json::{Value, json};

const ID: &str = "google_sheets";

pub fn definition() -> model::Provider {
    super::definition(
        ID,
        "Google Sheets",
        "Google Sheets integration for reading, writing, and managing spreadsheet data.",
        CATEGORY_SPREADSHEETS,
        &["spreadsheets", "spreadsheets.readonly"],
    )
}

pub struct GoogleSheets;

impl ToolProvider for GoogleSheets {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        let s = json!({"type":"string"});
        let grid = json!({"type":"array","items":{"type":"array","items":{"type":"string"}}});
        let index = json!({"type":"integer","minimum":0});
        vec![
            tool(
                ID,
                "google_sheets_read_range",
                "Read data from a cell range in A1 notation",
                "Read data from a cell range in A1 notation (e.g. \"Sheet1!A1:D10\").",
                READ,
                json!({"spreadsheet_id":s,"range":s}),
                &["spreadsheet_id", "range"],
            ),
            tool(
                ID,
                "google_sheets_write_range",
                "Write data to a cell range",
                "Write data to a cell range. Values are interpreted as if typed by the user (USER_ENTERED).",
                WRITE,
                json!({"spreadsheet_id":s,"range":s,"values":grid}),
                &["spreadsheet_id", "range", "values"],
            ),
            tool(
                ID,
                "google_sheets_append_rows",
                "Append rows to the end of a sheet's data",
                "Append rows to the end of a sheet's existing data.",
                WRITE,
                json!({"spreadsheet_id":s,"sheet_name":s,"rows":grid}),
                &["spreadsheet_id", "sheet_name", "rows"],
            ),
            tool(
                ID,
                "google_sheets_search_rows",
                "Find rows matching criteria in a column",
                "Search for rows where a given column matches a value using the specified operator (equals, contains, greater_than, less_than). The column can be identified by letter or by its header name in the first row.",
                READ,
                json!({
                    "spreadsheet_id":s,
                    "sheet_name":s,
                    "column":{"type":"string","description":"Column letter (e.g. \"A\") or header name from the first row."},
                    "operator":{"type":"string","description":"One of: \"equals\", \"contains\", \"greater_than\", \"less_than\"."},
                    "value":s,
                    "max_results":{"type":"integer","minimum":0,"description":"Maximum number of matches to return (default 50)."}
                }),
                &[
                    "spreadsheet_id",
                    "sheet_name",
                    "column",
                    "operator",
                    "value",
                ],
            ),
            tool(
                ID,
                "google_sheets_create_spreadsheet",
                "Create a new spreadsheet",
                "Create a new Google Sheets spreadsheet with an optional initial sheet name.",
                WRITE,
                json!({"title":s,"sheet_name":s}),
                &["title"],
            ),
            tool(
                ID,
                "google_sheets_list_sheets",
                "List all worksheets in a spreadsheet",
                "List all worksheet tabs and their properties in a spreadsheet.",
                READ,
                json!({"spreadsheet_id":s}),
                &["spreadsheet_id"],
            ),
            tool(
                ID,
                "google_sheets_add_sheet",
                "Add a new worksheet tab",
                "Add a new worksheet tab to an existing spreadsheet.",
                WRITE,
                json!({"spreadsheet_id":s,"sheet_name":s}),
                &["spreadsheet_id", "sheet_name"],
            ),
            tool(
                ID,
                "google_sheets_get_spreadsheet_info",
                "Get metadata about a spreadsheet",
                "Retrieve metadata about a spreadsheet including its title, URL, locale, and worksheet list.",
                READ,
                json!({"spreadsheet_id":s}),
                &["spreadsheet_id"],
            ),
            tool(
                ID,
                "google_sheets_clear_range",
                "Clear cell contents while preserving formatting",
                "Clear the contents of cells in a range while preserving formatting.",
                DELETE,
                json!({"spreadsheet_id":s,"range":s}),
                &["spreadsheet_id", "range"],
            ),
            tool(
                ID,
                "google_sheets_format_cells",
                "Apply formatting to cells",
                "Apply formatting (bold, italic, text colour, background colour) to a range of cells.",
                WRITE,
                json!({
                    "spreadsheet_id":s,
                    "sheet_id":index,
                    "start_row":index,
                    "end_row":index,
                    "start_column":index,
                    "end_column":index,
                    "bold":{"type":"boolean"},
                    "italic":{"type":"boolean"},
                    "text_color":{"type":"string","description":"Hex colour like \"#FF0000\""},
                    "background_color":{"type":"string","description":"Hex colour like \"#00FF00\""}
                }),
                &[
                    "spreadsheet_id",
                    "sheet_id",
                    "start_row",
                    "end_row",
                    "start_column",
                    "end_column",
                ],
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
            let api = Api::new(
                access,
                "google_sheets_api",
                "https://sheets.googleapis.com/v4",
            );
            let id = text(&input, "spreadsheet_id");
            let range = text(&input, "range");
            let batch = format!("{id}:batchUpdate");
            match name {
                "google_sheets_read_range" => {
                    let body = call(api.get(&["spreadsheets", id, "values", range])?).await?;
                    Ok(json!({
                        "range": body["range"].as_str().unwrap_or_default(),
                        "values": cells(&body["values"]),
                        "major_dimension": body["majorDimension"].as_str().unwrap_or("ROWS")
                    }))
                }
                "google_sheets_write_range" => {
                    let body = call(
                        api.request(Method::PUT, &["spreadsheets", id, "values", range])?
                            .query(&[("valueInputOption", "USER_ENTERED")])
                            .json(&json!({"values": input["values"]})),
                    )
                    .await?;
                    Ok(json!({
                        "updated_range": body["updatedRange"].as_str().unwrap_or_default(),
                        "updated_rows": body["updatedRows"].as_u64().unwrap_or(0),
                        "updated_columns": body["updatedColumns"].as_u64().unwrap_or(0),
                        "updated_cells": body["updatedCells"].as_u64().unwrap_or(0)
                    }))
                }
                "google_sheets_append_rows" => {
                    let sheet = format!("{}:append", text(&input, "sheet_name"));
                    let body = call(
                        api.post(&["spreadsheets", id, "values", &sheet])?
                            .query(&[
                                ("valueInputOption", "USER_ENTERED"),
                                ("insertDataOption", "INSERT_ROWS"),
                            ])
                            .json(&json!({"values": input["rows"]})),
                    )
                    .await?;
                    Ok(json!({
                        "updated_range": body["updates"]["updatedRange"].as_str().unwrap_or_default(),
                        "updated_rows": body["updates"]["updatedRows"].as_u64().unwrap_or(0)
                    }))
                }
                "google_sheets_search_rows" => {
                    let body = call(api.get(&[
                        "spreadsheets",
                        id,
                        "values",
                        text(&input, "sheet_name"),
                    ])?)
                    .await?;
                    search(&input, cells(&body["values"]))
                }
                "google_sheets_create_spreadsheet" => {
                    let mut body = json!({"properties": {"title": text(&input, "title")}});
                    if let Some(sheet) = optional(&input, "sheet_name") {
                        body["sheets"] = json!([{"properties": {"title": sheet}}]);
                    }
                    let created = call(api.post(&["spreadsheets"])?.json(&body)).await?;
                    Ok(json!({
                        "spreadsheet_id": created["spreadsheetId"].as_str().unwrap_or_default(),
                        "spreadsheet_url": created["spreadsheetUrl"].as_str().unwrap_or_default(),
                        "title": created["properties"]["title"].as_str().unwrap_or_default()
                    }))
                }
                "google_sheets_list_sheets" => {
                    let body = call(
                        api.get(&["spreadsheets", id])?
                            .query(&[("fields", "sheets.properties")]),
                    )
                    .await?;
                    Ok(json!({"sheets": sheets(&body)}))
                }
                "google_sheets_add_sheet" => {
                    let body = call(api.post(&["spreadsheets", &batch])?.json(&json!({
                        "requests": [{"addSheet": {"properties": {"title": text(&input, "sheet_name")}}}]
                    })))
                    .await?;
                    let added = &body["replies"][0]["addSheet"]["properties"];
                    Ok(json!({
                        "sheet_id": added["sheetId"].as_u64().unwrap_or(0),
                        "title": added["title"].as_str().unwrap_or_default()
                    }))
                }
                "google_sheets_get_spreadsheet_info" => {
                    let body = call(api.get(&["spreadsheets", id])?).await?;
                    Ok(json!({
                        "spreadsheet_id": body["spreadsheetId"].as_str().unwrap_or_default(),
                        "title": body["properties"]["title"].as_str().unwrap_or_default(),
                        "spreadsheet_url": body["spreadsheetUrl"].as_str().unwrap_or_default(),
                        "sheets": sheets(&body),
                        "locale": body["properties"]["locale"].as_str()
                    }))
                }
                "google_sheets_clear_range" => {
                    let clear = format!("{range}:clear");
                    let body = call(
                        api.post(&["spreadsheets", id, "values", &clear])?
                            .json(&json!({})),
                    )
                    .await?;
                    Ok(json!({"cleared_range": body["clearedRange"].as_str().unwrap_or_default()}))
                }
                "google_sheets_format_cells" => {
                    let mut format = json!({});
                    let mut fields = Vec::new();
                    for key in ["bold", "italic"] {
                        if let Some(value) = input[key].as_bool() {
                            format["textFormat"][key] = json!(value);
                            fields.push(format!("userEnteredFormat.textFormat.{key}"));
                        }
                    }
                    if let Some(hex) = optional(&input, "text_color") {
                        format["textFormat"]["foregroundColor"] = color(hex);
                        fields.push("userEnteredFormat.textFormat.foregroundColor".into());
                    }
                    if let Some(hex) = optional(&input, "background_color") {
                        format["backgroundColor"] = color(hex);
                        fields.push("userEnteredFormat.backgroundColor".into());
                    }
                    call(api.post(&["spreadsheets", &batch])?.json(&json!({
                        "requests": [{"repeatCell": {
                            "range": {
                                "sheetId": input["sheet_id"],
                                "startRowIndex": input["start_row"],
                                "endRowIndex": input["end_row"],
                                "startColumnIndex": input["start_column"],
                                "endColumnIndex": input["end_column"]
                            },
                            "cell": {"userEnteredFormat": format},
                            "fields": fields.join(",")
                        }}]
                    })))
                    .await?;
                    Ok(json!({"success": true}))
                }
                _ => Err(ConnectError::not_found("Unsupported Google Sheets tool")),
            }
        })
    }
}

/// Values as strings; non-string cells read as empty.
fn cells(values: &Value) -> Vec<Vec<String>> {
    values
        .as_array()
        .into_iter()
        .flatten()
        .map(|row| {
            row.as_array()
                .into_iter()
                .flatten()
                .map(|cell| cell.as_str().unwrap_or_default().to_owned())
                .collect()
        })
        .collect()
}

/// Matches data rows (after the header) on one column. Comparisons are numeric when both sides
/// parse as numbers, else lexical.
fn search(input: &Value, rows: Vec<Vec<String>>) -> ToolResult<Value> {
    let Some(header) = rows.first() else {
        return Ok(json!({"matches": [], "total_matches": 0}));
    };
    let column = text(input, "column");
    let letters = column.trim().to_uppercase();
    let index = if letters.len() <= 2 && letters.chars().all(|c| c.is_ascii_uppercase()) {
        letters
            .bytes()
            .fold(0usize, |index, c| index * 26 + (c - b'A' + 1) as usize)
            .checked_sub(1)
    } else {
        header
            .iter()
            .position(|name| name.to_lowercase() == column.to_lowercase())
    }
    .ok_or_else(|| {
        ConnectError::invalid_argument(format!(
            "Column '{column}' not found in header row or as a column letter"
        ))
    })?;
    let value = text(input, "value");
    let limit = input["max_results"].as_u64().unwrap_or(50) as usize;
    let compare = |cell: &str| match (cell.parse::<f64>(), value.parse::<f64>()) {
        (Ok(a), Ok(b)) => a.partial_cmp(&b),
        _ => Some(cell.cmp(value)),
    };
    let mut matches = Vec::new();
    let mut total = 0;
    for (i, row) in rows.iter().enumerate().skip(1) {
        let cell = row.get(index).map(String::as_str).unwrap_or_default();
        let hit = match text(input, "operator") {
            "equals" => cell == value,
            "contains" => cell.contains(value),
            "greater_than" => compare(cell) == Some(std::cmp::Ordering::Greater),
            "less_than" => compare(cell) == Some(std::cmp::Ordering::Less),
            _ => false,
        };
        if hit {
            total += 1;
            if matches.len() < limit {
                matches.push(json!({"row_number": i + 1, "values": row}));
            }
        }
    }
    Ok(json!({"matches": matches, "total_matches": total}))
}

fn sheets(body: &Value) -> Vec<Value> {
    body["sheets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|sheet| {
            let properties = sheet.get("properties")?;
            Some(json!({
                "sheet_id": properties["sheetId"].as_u64().unwrap_or(0),
                "title": properties["title"].as_str().unwrap_or_default(),
                "index": properties["index"].as_u64().unwrap_or(0),
                "row_count": properties["gridProperties"]["rowCount"].as_u64(),
                "column_count": properties["gridProperties"]["columnCount"].as_u64()
            }))
        })
        .collect()
}

/// `#RRGGBB` as Sheets' 0..1 RGB; unparseable channels are 0.
fn color(hex: &str) -> Value {
    let hex = hex.trim_start_matches('#');
    let channel = |at: usize| {
        hex.get(at..at + 2)
            .and_then(|pair| u8::from_str_radix(pair, 16).ok())
            .unwrap_or(0) as f32
            / 255.0
    };
    json!({"red": channel(0), "green": channel(2), "blue": channel(4)})
}
