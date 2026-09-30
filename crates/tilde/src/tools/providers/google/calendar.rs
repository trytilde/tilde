//! Google Calendar: events, free/busy slots, RSVPs and calendars, plus the current time for
//! relative scheduling.
use super::{Api, DELETE, READ, WRITE, call, optional, string_list, text, tool};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{categories::CATEGORY_CALENDAR, model};
use crate::proto::tilde::types::v1 as types;
use crate::tools::providers::ToolProvider;
use chrono::{DateTime, Duration, FixedOffset, Utc};
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use reqwest::Method;
use serde_json::{Value, json};

const ID: &str = "google_calendar";

pub fn definition() -> model::Provider {
    super::definition(
        ID,
        "Google Calendar",
        "Google Calendar integration for managing events, scheduling, and calendar access.",
        CATEGORY_CALENDAR,
        &["calendar", "calendar.events", "calendar.readonly"],
    )
}

pub struct GoogleCalendar;

impl ToolProvider for GoogleCalendar {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        let s = json!({"type":"string"});
        let emails = json!({"type":"array","items":{"type":"string"}});
        let count = json!({"type":"integer","minimum":0});
        vec![
            tool(
                ID,
                "google_calendar_create_event",
                "Create a calendar event",
                "Create a calendar event with attendees, location, description, and optional Google Meet link",
                WRITE,
                json!({"summary":s,"start":s,"end":s,"attendees":emails,"location":s,"description":s,"calendar_id":s,"create_meet_link":{"type":"boolean"}}),
                &["summary", "start", "end"],
            ),
            tool(
                ID,
                "google_calendar_find_events",
                "Search calendar events",
                "Search events by text query within a time range",
                READ,
                json!({"query":s,"time_min":s,"time_max":s,"calendar_id":s,"max_results":count}),
                &["query"],
            ),
            tool(
                ID,
                "google_calendar_list_events",
                "List upcoming events",
                "List upcoming events on a calendar. Defaults to next 7 days.",
                READ,
                json!({"calendar_id":s,"time_min":s,"time_max":s,"max_results":count}),
                &[],
            ),
            tool(
                ID,
                "google_calendar_find_free_slots",
                "Find free time slots",
                "Find available time slots across calendars within a date range",
                READ,
                json!({"calendars":emails,"time_min":s,"time_max":s,"duration_minutes":count,"timezone":s}),
                &["time_min", "time_max", "duration_minutes"],
            ),
            tool(
                ID,
                "google_calendar_update_event",
                "Update a calendar event",
                "Update an existing calendar event",
                WRITE,
                json!({"event_id":s,"calendar_id":s,"summary":s,"start":s,"end":s,"location":s,"description":s,"attendees":emails}),
                &["event_id"],
            ),
            tool(
                ID,
                "google_calendar_delete_event",
                "Delete a calendar event",
                "Delete a calendar event",
                DELETE,
                json!({"event_id":s,"calendar_id":s}),
                &["event_id"],
            ),
            tool(
                ID,
                "google_calendar_rsvp_event",
                "RSVP to a calendar event",
                "Respond to a calendar event invitation",
                WRITE,
                json!({"event_id":s,"calendar_id":s,"response":{"enum":["accepted","declined","tentative"]}}),
                &["event_id", "response"],
            ),
            tool(
                ID,
                "google_calendar_list_calendars",
                "List calendars",
                "List all calendars the user has access to",
                READ,
                json!({}),
                &[],
            ),
            tool(
                ID,
                "google_calendar_get_current_datetime",
                "Get current date and time",
                "Get current date and time. Essential for relative scheduling since agents have no clock.",
                READ,
                json!({"timezone":{"type":"string","description":"`UTC` or an offset such as `+05:30`."}}),
                &[],
            ),
        ]
    }
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        call_id: uuid::Uuid,
        name: &'a str,
        input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            let api = Api::new(
                access,
                "google_calendar_api",
                "https://www.googleapis.com/calendar/v3",
            );
            let calendar = optional(&input, "calendar_id").unwrap_or("primary");
            let event = ["calendars", calendar, "events", text(&input, "event_id")];
            match name {
                "google_calendar_create_event" => {
                    let mut body = json!({
                        "summary": text(&input, "summary"),
                        "start": {"dateTime": text(&input, "start")},
                        "end": {"dateTime": text(&input, "end")}
                    });
                    event_fields(&input, &mut body);
                    if input["create_meet_link"].as_bool().unwrap_or(false) {
                        // The call ID makes a retried call reuse the same conference request.
                        body["conferenceData"] =
                            json!({"createRequest": {"requestId": call_id.to_string()}});
                    }
                    let created = call(
                        api.post(&["calendars", calendar, "events"])?
                            .query(&[("conferenceDataVersion", "1")])
                            .json(&body),
                    )
                    .await?;
                    let meet = created["conferenceData"]["entryPoints"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .find(|entry| entry["entryPointType"].as_str() == Some("video"))
                        .and_then(|entry| entry["uri"].as_str());
                    Ok(json!({
                        "event_id": created["id"].as_str().unwrap_or_default(),
                        "html_link": created["htmlLink"].as_str().unwrap_or_default(),
                        "meet_link": meet
                    }))
                }
                "google_calendar_find_events" => {
                    let mut query = vec![
                        ("q", text(&input, "query").to_owned()),
                        (
                            "maxResults",
                            input["max_results"].as_u64().unwrap_or(10).to_string(),
                        ),
                        ("singleEvents", "true".into()),
                        ("orderBy", "startTime".into()),
                    ];
                    for (key, field) in [("timeMin", "time_min"), ("timeMax", "time_max")] {
                        if let Some(value) = optional(&input, field) {
                            query.push((key, value.to_owned()));
                        }
                    }
                    events(&api, calendar, &query).await
                }
                "google_calendar_list_events" => {
                    let now = Utc::now();
                    let query = [
                        (
                            "timeMin",
                            optional(&input, "time_min")
                                .map_or_else(|| now.to_rfc3339(), str::to_owned),
                        ),
                        (
                            "timeMax",
                            optional(&input, "time_max").map_or_else(
                                || (now + Duration::days(7)).to_rfc3339(),
                                str::to_owned,
                            ),
                        ),
                        (
                            "maxResults",
                            input["max_results"].as_u64().unwrap_or(25).to_string(),
                        ),
                        ("singleEvents", "true".into()),
                        ("orderBy", "startTime".into()),
                    ];
                    events(&api, calendar, &query).await
                }
                "google_calendar_find_free_slots" => free_slots(&api, &input).await,
                "google_calendar_update_event" => {
                    let mut body = json!({});
                    if let Some(summary) = optional(&input, "summary") {
                        body["summary"] = json!(summary);
                    }
                    for field in ["start", "end"] {
                        if let Some(value) = optional(&input, field) {
                            body[field] = json!({"dateTime": value});
                        }
                    }
                    event_fields(&input, &mut body);
                    let updated = call(api.request(Method::PATCH, &event)?.json(&body)).await?;
                    Ok(json!({
                        "event_id": updated["id"].as_str().unwrap_or_default(),
                        "html_link": updated["htmlLink"].as_str().unwrap_or_default()
                    }))
                }
                "google_calendar_delete_event" => {
                    call(api.request(Method::DELETE, &event)?).await?;
                    Ok(json!({"success": true}))
                }
                "google_calendar_rsvp_event" => {
                    let mut found = call(api.get(&event)?).await?;
                    let attendees = found["attendees"]
                        .as_array_mut()
                        .ok_or_else(|| ConnectError::unknown("Event has no attendees"))?;
                    let me = attendees
                        .iter_mut()
                        .find(|attendee| attendee["self"].as_bool().unwrap_or(false))
                        .ok_or_else(|| {
                            ConnectError::unknown(
                                "Could not find authenticated user in event attendees",
                            )
                        })?;
                    me["responseStatus"] = json!(text(&input, "response"));
                    call(
                        api.request(Method::PATCH, &event)?
                            .json(&json!({"attendees": found["attendees"]})),
                    )
                    .await?;
                    Ok(json!({"success": true}))
                }
                "google_calendar_list_calendars" => {
                    let list = call(api.get(&["users", "me", "calendarList"])?).await?;
                    let calendars: Vec<Value> = list["items"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|item| {
                            json!({
                                "id": item["id"].as_str().unwrap_or_default(),
                                "summary": item["summary"].as_str().unwrap_or_default(),
                                "primary": item["primary"].as_bool().unwrap_or(false),
                                "access_role": item["accessRole"].as_str().unwrap_or_default()
                            })
                        })
                        .collect();
                    Ok(json!({"calendars": calendars}))
                }
                "google_calendar_get_current_datetime" => {
                    let zone = optional(&input, "timezone").unwrap_or("UTC");
                    // Named zones other than UTC are not resolved; they fall back to UTC.
                    let now = match offset(zone) {
                        Some(offset) => Utc::now().with_timezone(&offset),
                        None => Utc::now().fixed_offset(),
                    };
                    Ok(json!({
                        "datetime": now.to_rfc3339(),
                        "date": now.format("%Y-%m-%d").to_string(),
                        "time": now.format("%H:%M:%S").to_string(),
                        "day_of_week": now.format("%A").to_string(),
                        "timezone": zone
                    }))
                }
                _ => Err(ConnectError::not_found("Unsupported Google Calendar tool")),
            }
        })
    }
}

/// Location, description and attendees shared by create and update.
fn event_fields(input: &Value, body: &mut Value) {
    for field in ["location", "description"] {
        if let Some(value) = optional(input, field) {
            body[field] = json!(value);
        }
    }
    if input["attendees"].is_array() {
        body["attendees"] = string_list(&input["attendees"])
            .into_iter()
            .map(|email| json!({"email": email}))
            .collect();
    }
}

async fn events(api: &Api<'_>, calendar: &str, query: &[(&str, String)]) -> ToolResult<Value> {
    let list = call(api.get(&["calendars", calendar, "events"])?.query(query)).await?;
    let events: Vec<Value> = list["items"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|event| {
            let when = |key: &str| {
                event[key]["dateTime"]
                    .as_str()
                    .or_else(|| event[key]["date"].as_str())
            };
            let attendees: Vec<Value> = event["attendees"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|attendee| {
                    json!({
                        "email": attendee["email"].as_str().unwrap_or_default(),
                        "response_status": attendee["responseStatus"].as_str(),
                        "organizer": attendee["organizer"].as_bool().unwrap_or(false),
                        "self": attendee["self"].as_bool().unwrap_or(false)
                    })
                })
                .collect();
            json!({
                "id": event["id"].as_str().unwrap_or_default(),
                "summary": event["summary"].as_str(),
                "start": when("start"),
                "end": when("end"),
                "location": event["location"].as_str(),
                "description": event["description"].as_str(),
                "html_link": event["htmlLink"].as_str(),
                "status": event["status"].as_str(),
                "organizer": event["organizer"]["email"].as_str(),
                "attendees": attendees
            })
        })
        .collect();
    Ok(json!({"events": events}))
}

/// Merges every calendar's busy periods and returns the gaps in the range at least
/// `duration_minutes` long.
async fn free_slots(api: &Api<'_>, input: &Value) -> ToolResult<Value> {
    let calendars = match string_list(&input["calendars"]) {
        list if input["calendars"].is_array() => list,
        _ => vec!["primary".to_owned()],
    };
    let parse = |key: &str| {
        DateTime::parse_from_rfc3339(text(input, key))
            .map_err(|error| ConnectError::invalid_argument(format!("Invalid {key}: {error}")))
    };
    let body = json!({
        "timeMin": text(input, "time_min"),
        "timeMax": text(input, "time_max"),
        "items": calendars.iter().map(|id| json!({"id": id})).collect::<Vec<_>>()
    });
    let response = call(api.post(&["freeBusy"])?.json(&body)).await?;
    let mut busy: Vec<(DateTime<FixedOffset>, DateTime<FixedOffset>)> = response["calendars"]
        .as_object()
        .into_iter()
        .flat_map(|calendars| calendars.values())
        .flat_map(|calendar| calendar["busy"].as_array().into_iter().flatten())
        .filter_map(|period| {
            Some((
                DateTime::parse_from_rfc3339(period["start"].as_str()?).ok()?,
                DateTime::parse_from_rfc3339(period["end"].as_str()?).ok()?,
            ))
        })
        .collect();
    busy.sort_by_key(|(start, _)| *start);
    let mut merged: Vec<(DateTime<FixedOffset>, DateTime<FixedOffset>)> = Vec::new();
    for (start, end) in busy {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    let duration = Duration::minutes(input["duration_minutes"].as_i64().unwrap_or(0));
    let (mut cursor, range_end) = (parse("time_min")?, parse("time_max")?);
    let mut slots = Vec::new();
    for (start, end) in merged {
        if cursor + duration <= start {
            slots.push(json!({"start": cursor.to_rfc3339(), "end": start.to_rfc3339()}));
        }
        cursor = cursor.max(end);
    }
    if cursor + duration <= range_end {
        slots.push(json!({"start": cursor.to_rfc3339(), "end": range_end.to_rfc3339()}));
    }
    Ok(json!({"free_slots": slots}))
}

/// `+HH:MM`, `-HHMM` or `+HH`.
fn offset(zone: &str) -> Option<FixedOffset> {
    let zone = zone.trim();
    let sign = match zone.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let rest = &zone[1..];
    let (hours, minutes) = match rest.split_once(':') {
        Some((hours, minutes)) => (hours.parse::<i32>().ok()?, minutes.parse::<i32>().ok()?),
        None if rest.len() == 4 => (rest[..2].parse().ok()?, rest[2..].parse().ok()?),
        None if rest.len() == 2 => (rest.parse().ok()?, 0),
        None => return None,
    };
    FixedOffset::east_opt(sign * (hours * 3600 + minutes * 60))
}
