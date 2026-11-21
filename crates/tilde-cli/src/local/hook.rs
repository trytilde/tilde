//! What a harness's hooks run. Claude Code and Codex both hand a hook the session's id and the
//! path of its transcript on stdin. At the start of a session the agent's skills are brought up
//! to date; when a turn or the session ends, the part of the transcript not yet reported is
//! sent to Tilde.
//!
//! The cursor is the byte offset of the last whole line reported, kept per session. It moves
//! only after Tilde accepted the events, and Tilde records an event once however often it is
//! sent, so a failed or interrupted hook loses nothing: the next one sends from the same place.
use super::{Connection, Harness, install, skills, state_dir, transcript};
use crate::api::{failed, options};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;
use tilde_contracts::proto::tilde::local::v1 as wire;

/// The engine takes at most 500 events a request.
const BATCH: usize = 200;

#[derive(Deserialize)]
struct Payload {
    session_id: String,
    transcript_path: Option<String>,
    hook_event_name: String,
}

pub async fn run(harness: Harness) -> Result<()> {
    let payload: Payload = serde_json::from_reader(std::io::stdin().lock())
        .context("The hook payload is not the JSON a harness sends")?;
    let connection = Connection::load(harness)?;
    if payload.hook_event_name == "SessionStart" {
        return skills::sync(&connection, &install::harness_dir(harness)?.join("skills")).await;
    }
    let path = payload
        .transcript_path
        .context("The hook payload names no transcript")?;
    let cursors = state_dir()?.join("sessions").join(harness.name());
    report(
        &connection,
        harness,
        &payload.session_id,
        Path::new(&path),
        &cursors,
    )
    .await
}

/// Send what `transcript` gained since the session's cursor in `cursors`, then move the cursor.
async fn report(
    connection: &Connection,
    harness: Harness,
    session: &str,
    transcript: &Path,
    cursors: &Path,
) -> Result<()> {
    let text = std::fs::read_to_string(transcript)
        .with_context(|| format!("Could not read the transcript {}", transcript.display()))?;
    // A line still being written is left for the next hook.
    let whole = text.rfind('\n').map_or(0, |end| end + 1);
    let cursor = cursors.join(file_name(session));
    let from = std::fs::read_to_string(&cursor)
        .ok()
        .and_then(|offset| offset.trim().parse::<usize>().ok())
        .filter(|offset| *offset <= whole)
        .unwrap_or(0);
    let parsed = transcript::parse(harness, &text[..whole], from);
    let client = connection.client()?;
    for events in parsed.events.chunks(BATCH) {
        client
            .ingest_with_options(
                wire::IngestRequest {
                    session_id: session.to_owned(),
                    title: parsed.title.clone(),
                    events: events.to_vec(),
                    ..Default::default()
                },
                options(),
            )
            .await
            .map_err(|error| failed("Ingest", error))?;
    }
    std::fs::create_dir_all(cursor.parent().context("cursor directory")?)?;
    std::fs::write(&cursor, whole.to_string())?;
    Ok(())
}

/// A session id as a file name: harnesses use UUIDs, and anything else is kept to safe characters.
fn file_name(session: &str) -> String {
    session
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router, body::Bytes, http::HeaderMap, http::header, response::IntoResponse, routing::post,
    };
    use buffa::Message;
    use std::sync::{Arc, Mutex};
    use tilde_contracts::proto::tilde::runtime::v1 as runtime;

    #[derive(Default)]
    struct Gateway {
        /// Each accepted Ingest: the session and the ids of its events.
        ingested: Vec<(String, Vec<String>)>,
        /// Answer the next Ingest with an error.
        failing: bool,
        skills: Vec<runtime::SkillSummary>,
        /// The skill files asked for.
        read: Vec<String>,
    }
    fn proto<M: Message>(message: M) -> axum::response::Response {
        (
            [(header::CONTENT_TYPE, "application/proto")],
            message.encode_to_vec(),
        )
            .into_response()
    }
    /// A stand-in for the agent's ingress, over the real wire format: it answers only the
    /// agent's key.
    async fn gateway(state: Arc<Mutex<Gateway>>) -> Connection {
        let keyed = |headers: &HeaderMap| {
            headers
                .get(header::AUTHORIZATION)
                .is_some_and(|v| v == "Bearer tilde_chat_key")
        };
        let service = "/agents/a1/tilde.local.v1.LocalAgentService";
        let app = Router::new()
            .route(
                &format!("{service}/Ingest"),
                post({
                    let state = state.clone();
                    move |headers: HeaderMap, body: Bytes| async move {
                        let mut state = state.lock().unwrap();
                        if !keyed(&headers) || std::mem::take(&mut state.failing) {
                            return http::StatusCode::SERVICE_UNAVAILABLE.into_response();
                        }
                        let request = wire::IngestRequest::decode_from_slice(&body).unwrap();
                        assert_eq!(request.title, "bring it back up");
                        let ids = request.events.iter().map(|e| e.id.clone()).collect();
                        state.ingested.push((request.session_id, ids));
                        proto(wire::IngestResponse::default())
                    }
                }),
            )
            .route(
                &format!("{service}/ListSkills"),
                post({
                    let state = state.clone();
                    move || async move {
                        proto(wire::ListSkillsResponse {
                            skills: state.lock().unwrap().skills.clone(),
                            ..Default::default()
                        })
                    }
                }),
            )
            .route(
                &format!("{service}/ReadSkillFile"),
                post({
                    let state = state.clone();
                    move |body: Bytes| async move {
                        let request = wire::ReadSkillFileRequest::decode_from_slice(&body).unwrap();
                        let file = format!("{}:{}", request.name, request.path);
                        state.lock().unwrap().read.push(file.clone());
                        proto(wire::ReadSkillFileResponse {
                            content: format!("content of {file}"),
                            ..Default::default()
                        })
                    }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Connection {
            url,
            agent_id: "a1".into(),
            key: "tilde_chat_key".into(),
        }
    }

    /// A hook sends only what the transcript gained, and the cursor moves only once Tilde took
    /// it: a refused report is sent again by the next hook, and a line still being written
    /// waits for the one after.
    #[tokio::test]
    async fn each_hook_reports_what_is_new_and_a_refused_report_is_sent_again() {
        let state = Arc::new(Mutex::new(Gateway::default()));
        let connection = gateway(state.clone()).await;
        let dir = tempfile::tempdir().unwrap();
        let (transcript, cursors) = (dir.path().join("session.jsonl"), dir.path().join("cursors"));
        let prompt = |id: &str, text: &str| {
            format!(
                r#"{{"type":"user","uuid":"{id}","timestamp":"2026-11-21T10:00:00Z","message":{{"role":"user","content":"{text}"}}}}"#
            )
        };
        let answer = |id: &str| {
            format!(
                r#"{{"type":"assistant","uuid":"{id}","timestamp":"2026-11-21T10:00:05Z","message":{{"role":"assistant","content":[{{"type":"text","text":"done"}}]}}}}"#
            )
        };
        let run = || {
            report(
                &connection,
                Harness::ClaudeCode,
                "s1",
                &transcript,
                &cursors,
            )
        };
        let ingested = || state.lock().unwrap().ingested.clone();
        let turn = |ids: &[&str]| {
            (
                "s1".to_owned(),
                ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
            )
        };

        std::fs::write(
            &transcript,
            format!("{}\n{}\n", prompt("u1", "bring it back up"), answer("a1")),
        )
        .unwrap();
        state.lock().unwrap().failing = true;
        assert!(run().await.is_err(), "the gateway refused the report");
        run().await.unwrap();
        assert_eq!(ingested(), [turn(&["u1", "a1"])]);

        // The second turn's answer is still being written when the hook runs.
        let second = format!("{}\n{}", prompt("u2", "thanks"), answer("a2"));
        std::fs::write(
            &transcript,
            format!("{}{second}", std::fs::read_to_string(&transcript).unwrap()),
        )
        .unwrap();
        run().await.unwrap();
        assert_eq!(ingested()[1..], [turn(&["u2"])]);
        std::fs::write(
            &transcript,
            format!("{}\n", std::fs::read_to_string(&transcript).unwrap()),
        )
        .unwrap();
        run().await.unwrap();
        run().await.unwrap();
        assert_eq!(
            ingested()[2..],
            [turn(&["a2"])],
            "nothing new, nothing sent"
        );
    }

    /// The agent's skills land in the harness's skill folder at their version, are rewritten
    /// only when the version moves, and go when the agent loses them; a skill the person put
    /// there is never touched.
    #[tokio::test]
    async fn skills_follow_the_agent_and_leave_the_persons_own_alone() {
        let state = Arc::new(Mutex::new(Gateway::default()));
        let connection = gateway(state.clone()).await;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("skills");
        std::fs::create_dir_all(root.join("mine")).unwrap();
        std::fs::write(root.join("mine/SKILL.md"), "my own").unwrap();
        let skill = |name: &str, version: &str| runtime::SkillSummary {
            name: name.into(),
            source: "team".into(),
            version_id: version.into(),
            files: vec![
                runtime::SkillFileInfo {
                    path: "SKILL.md".into(),
                    ..Default::default()
                },
                runtime::SkillFileInfo {
                    path: "scripts/run.sh".into(),
                    executable: true,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let read = || std::mem::take(&mut state.lock().unwrap().read);

        state.lock().unwrap().skills = vec![skill("greet", "v1"), skill("mine", "v1")];
        skills::sync(&connection, &root).await.unwrap();
        assert_eq!(read(), ["team/greet:SKILL.md", "team/greet:scripts/run.sh"]);
        assert_eq!(
            std::fs::read_to_string(root.join("greet/SKILL.md")).unwrap(),
            "content of team/greet:SKILL.md"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(root.join("greet/scripts/run.sh"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111);
        }
        assert_eq!(
            std::fs::read_to_string(root.join("mine/SKILL.md")).unwrap(),
            "my own"
        );

        skills::sync(&connection, &root).await.unwrap();
        assert!(
            read().is_empty(),
            "an installed version is not downloaded again"
        );
        state.lock().unwrap().skills = vec![skill("greet", "v2")];
        skills::sync(&connection, &root).await.unwrap();
        assert_eq!(read().len(), 2);

        state.lock().unwrap().skills = vec![];
        skills::sync(&connection, &root).await.unwrap();
        assert!(!root.join("greet").exists());
        assert!(root.join("mine/SKILL.md").exists());
    }
}
