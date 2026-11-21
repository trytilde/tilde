//! Reading a harness's transcript as the events Tilde records: what the person said, what the
//! agent said, and each finished tool call. Both harnesses append JSON lines to one file per
//! session; these formats are the harnesses' own and undocumented, so a line that does not
//! read as expected is skipped, never an error.
//!
//! The whole file is read each time, because a tool's result names a call made on an earlier
//! line and a turn begins at an earlier prompt; only events on lines at or after `from` (a
//! byte offset) are returned.
use super::Harness;
use serde_json::Value;
use std::collections::HashMap;
use tilde_contracts::proto::tilde::local::v1 as wire;

/// Tool payloads are cut to this, as the contract says; messages to the engine's megabyte.
const MAX_TOOL: usize = 64 * 1024;
const MAX_TEXT: usize = 1024 * 1024 - 64;

pub struct Parsed {
    /// What the session is about: the harness's own title, else the first prompt.
    pub title: String,
    pub events: Vec<wire::Event>,
}

pub fn parse(harness: Harness, text: &str, from: usize) -> Parsed {
    let mut session = Session {
        from,
        offset: 0,
        turn: "session".into(),
        title: String::new(),
        events: Vec::new(),
    };
    let mut calls = HashMap::new();
    let mut model = String::new();
    for line in text.split_inclusive('\n') {
        if let Ok(value) = serde_json::from_str::<Value>(line) {
            match harness {
                Harness::Codex => codex(&mut session, &mut model, &value),
                _ => claude_code(&mut session, &mut calls, &value),
            }
        }
        session.offset += line.len();
    }
    Parsed {
        title: session.title,
        events: session.events,
    }
}

struct Session {
    from: usize,
    /// Where the line being read starts.
    offset: usize,
    /// The id of the prompt that began the current turn.
    turn: String,
    title: String,
    events: Vec<wire::Event>,
}
impl Session {
    fn push(&mut self, id: &str, at: &Value, detail: wire::event::Detail) {
        if self.offset < self.from || id.is_empty() {
            return;
        }
        self.events.push(wire::Event {
            id: id.to_owned(),
            turn_id: self.turn.clone(),
            at: time(at).into(),
            detail: Some(detail),
            ..Default::default()
        });
    }
    fn prompt(&mut self, id: &str, at: &Value, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        if self.title.is_empty() {
            self.title = text
                .trim()
                .lines()
                .next()
                .unwrap_or_default()
                .chars()
                .take(80)
                .collect();
        }
        self.turn = id.to_owned();
        self.push(
            id,
            at,
            wire::UserMessage {
                text: cut(text, MAX_TEXT),
                ..Default::default()
            }
            .into(),
        );
    }
    fn answer(&mut self, id: &str, at: &Value, text: &str, model: &str) {
        if text.trim().is_empty() {
            return;
        }
        let message = wire::AgentMessage {
            text: cut(text, MAX_TEXT),
            model: model.to_owned(),
            ..Default::default()
        };
        self.push(id, at, message.into());
    }
    fn tool(&mut self, id: &str, at: &Value, call: wire::ToolCall) {
        self.push(id, at, call.into());
    }
}

/// A finished tool call, its payloads cut to what the contract takes.
fn tool_call(
    started: &Value,
    name: &str,
    input: &str,
    output: &str,
    failed: bool,
) -> wire::ToolCall {
    wire::ToolCall {
        name: name.chars().take(200).collect(),
        input: cut(input, MAX_TOOL),
        output: cut(output, MAX_TOOL),
        failed,
        started_at: time(started).into(),
        ..Default::default()
    }
}

/// An RFC 3339 string, or milliseconds since the epoch.
fn time(value: &Value) -> Option<buffa_types::google::protobuf::Timestamp> {
    let at = match value {
        Value::String(text) => chrono::DateTime::parse_from_rfc3339(text).ok()?.to_utc(),
        Value::Number(millis) => chrono::DateTime::from_timestamp_millis(millis.as_i64()?)?,
        _ => return None,
    };
    Some(buffa_types::google::protobuf::Timestamp {
        seconds: at.timestamp(),
        nanos: at.timestamp_subsec_nanos() as i32,
        ..Default::default()
    })
}
fn cut(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_owned();
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[truncated]", &text[..end])
}
/// A JSON value as a tool's input or output: text as it is, anything else as JSON.
fn payload(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}
/// The text of content blocks (`[{"type": "text", "text": ...}]`) of the given types, joined.
fn blocks(content: &Value, types: &[&str]) -> String {
    content
        .as_array()
        .into_iter()
        .flatten()
        .filter(|block| types.contains(&block["type"].as_str().unwrap_or_default()))
        .filter_map(|block| block["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Claude Code (`~/.claude/projects/<project>/<session>.jsonl`): `user` and `assistant` lines
/// carry an API message. A person's prompt is a `user` line with text; a `tool_use` block of an
/// assistant line is answered by a `tool_result` block of a later `user` line. Subagent
/// (sidechain) lines and the harness's own notes (`isMeta`, slash commands) are not the
/// conversation.
fn claude_code(
    session: &mut Session,
    calls: &mut HashMap<String, (String, String, Value)>,
    line: &Value,
) {
    if let Some(title) = line["aiTitle"].as_str() {
        session.title = title.chars().take(80).collect();
    }
    if line["isSidechain"] == true || line["isMeta"] == true {
        return;
    }
    let (id, at) = (
        line["uuid"].as_str().unwrap_or_default(),
        &line["timestamp"],
    );
    let content = &line["message"]["content"];
    match line["type"].as_str() {
        Some("user") => {
            let text = match content {
                Value::String(text) => text.clone(),
                _ => blocks(content, &["text"]),
            };
            if !text.starts_with("<command-") && !text.starts_with("<local-command-") {
                session.prompt(id, at, &text);
            }
            for block in content.as_array().into_iter().flatten() {
                let call = block["tool_use_id"].as_str().unwrap_or_default();
                if block["type"] != "tool_result" {
                    continue;
                }
                let Some((name, input, started)) = calls.remove(call) else {
                    continue;
                };
                let output = match &block["content"] {
                    Value::Array(_) => blocks(&block["content"], &["text"]),
                    other => payload(other),
                };
                session.tool(
                    call,
                    at,
                    tool_call(&started, &name, &input, &output, block["is_error"] == true),
                );
            }
        }
        Some("assistant") => {
            for (index, block) in content.as_array().into_iter().flatten().enumerate() {
                match block["type"].as_str() {
                    Some("text") => {
                        // A line is one block in practice; a second text block needs its own id.
                        let id = if index == 0 {
                            id.to_owned()
                        } else {
                            format!("{id}:{index}")
                        };
                        let model = line["message"]["model"].as_str().unwrap_or_default();
                        session.answer(&id, at, block["text"].as_str().unwrap_or_default(), model);
                    }
                    Some("tool_use") => {
                        if let (Some(call), Some(name)) =
                            (block["id"].as_str(), block["name"].as_str())
                        {
                            calls.insert(
                                call.to_owned(),
                                (name.to_owned(), payload(&block["input"]), at.clone()),
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

/// Codex (`~/.codex/sessions/<date>/rollout-<time>-<session>.jsonl`): each finished item of a
/// turn is an `event_msg` line of type `item_completed`. Messages, commands, MCP calls, file
/// changes and extensions (web search) are the conversation; reasoning and bookkeeping items
/// are not. The model is named by the turn's `turn_context` line.
fn codex(session: &mut Session, model: &mut String, line: &Value) {
    let payload_of = &line["payload"];
    if line["type"] == "turn_context" {
        if let Some(name) = payload_of["model"].as_str() {
            *model = name.to_owned();
        }
        return;
    }
    if line["type"] != "event_msg" || payload_of["type"] != "item_completed" {
        return;
    }
    let item = &payload_of["item"];
    let id = item["id"].as_str().unwrap_or_default();
    let at = match &payload_of["completed_at_ms"] {
        Value::Null => &line["timestamp"],
        millis => millis,
    };
    let started = &payload_of["started_at_ms"];
    let finished = item["status"]
        .as_str()
        .is_none_or(|status| status == "completed");
    match item["type"].as_str() {
        Some("UserMessage") => session.prompt(id, at, &blocks(&item["content"], &["text"])),
        Some("AgentMessage") => {
            session.answer(id, at, &blocks(&item["content"], &["Text", "text"]), model)
        }
        Some("CommandExecution") => {
            // `["/bin/bash", "-lc", "<script>"]`: the script is what the agent ran.
            let command = item["command"]
                .as_array()
                .and_then(|c| c.last())
                .map(payload)
                .unwrap_or_default();
            let failed = !finished || item["exit_code"].as_i64().is_some_and(|code| code != 0);
            session.tool(
                id,
                at,
                tool_call(
                    started,
                    "shell",
                    &command,
                    &payload(&item["aggregated_output"]),
                    failed,
                ),
            );
        }
        Some("McpToolCall") => {
            let name = format!(
                "{}.{}",
                item["server"].as_str().unwrap_or_default(),
                item["tool"].as_str().unwrap_or_default()
            );
            session.tool(
                id,
                at,
                tool_call(
                    started,
                    &name,
                    &payload(&item["arguments"]),
                    &payload(&item["result"]),
                    !finished,
                ),
            );
        }
        Some("FileChange") => session.tool(
            id,
            at,
            tool_call(
                started,
                "apply_patch",
                &payload(&item["changes"]),
                &payload(&item["stdout"]),
                !finished,
            ),
        ),
        Some("Extension") => {
            let name = item["kind"].as_str().unwrap_or("extension");
            session.tool(
                id,
                at,
                tool_call(started, name, &payload(&item["query"]), "", false),
            );
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wire::event::Detail;

    /// Each event as (id, turn, kind, text or tool name).
    fn summary(parsed: &Parsed) -> Vec<(String, String, &'static str, String)> {
        parsed
            .events
            .iter()
            .map(|event| {
                let (kind, text) = match event.detail.as_ref().unwrap() {
                    Detail::User(m) => ("user", m.text.clone()),
                    Detail::Agent(m) => ("agent", m.text.clone()),
                    Detail::Tool(t) => (
                        "tool",
                        format!(
                            "{} {} -> {}{}",
                            t.name,
                            t.input,
                            t.output,
                            if t.failed { " (failed)" } else { "" }
                        ),
                    ),
                };
                (event.id.clone(), event.turn_id.clone(), kind, text)
            })
            .collect()
    }
    fn row(
        id: &str,
        turn: &str,
        kind: &'static str,
        text: &str,
    ) -> (String, String, &'static str, String) {
        (id.into(), turn.into(), kind, text.into())
    }

    const CLAUDE: &str = concat!(
        r#"{"type":"mode","mode":"normal","sessionId":"s"}"#,
        "\n",
        r#"{"type":"user","uuid":"u1","timestamp":"2026-11-20T16:33:55.928Z","isSidechain":false,"message":{"role":"user","content":"bring the service back up\nit went down"}}"#,
        "\n",
        r#"{"type":"user","uuid":"m1","timestamp":"2026-11-20T16:33:56Z","isMeta":true,"message":{"role":"user","content":"<system-reminder>context</system-reminder>"}}"#,
        "\n",
        r#"{"type":"assistant","uuid":"a0","timestamp":"2026-11-20T16:34:00Z","message":{"role":"assistant","model":"claude-opus-5-5","content":[{"type":"thinking","thinking":"hm"}]}}"#,
        "\n",
        r#"{"type":"assistant","uuid":"a1","timestamp":"2026-11-20T16:34:01Z","message":{"role":"assistant","model":"claude-opus-5-5","content":[{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"systemctl status t3"}}]}}"#,
        "\n",
        r#"{"type":"assistant","uuid":"side","isSidechain":true,"timestamp":"2026-11-20T16:34:02Z","message":{"role":"assistant","content":[{"type":"text","text":"subagent chatter"}]}}"#,
        "\n",
        r#"{"type":"user","uuid":"r1","timestamp":"2026-11-20T16:34:05Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_1","content":[{"type":"text","text":"inactive (dead)"}],"is_error":true}]}}"#,
        "\n",
        r#"{"type":"assistant","uuid":"a2","timestamp":"2026-11-20T16:34:09Z","message":{"role":"assistant","model":"claude-opus-5-5","content":[{"type":"text","text":"It was stopped; I restarted it."}]}}"#,
        "\n",
        r#"{"type":"ai-title","aiTitle":"T3code service restart","sessionId":"s"}"#,
        "\n",
        r#"{"type":"user","uuid":"c1","timestamp":"2026-11-20T16:40:00Z","message":{"role":"user","content":"<command-name>/model</command-name>"}}"#,
        "\n",
        r#"{"type":"user","uuid":"u2","timestamp":"2026-11-20T16:41:00Z","message":{"role":"user","content":[{"type":"text","text":"thanks"}]}}"#,
        "\n",
    );

    #[test]
    fn claude_code_prompts_answers_and_paired_tool_calls() {
        let parsed = parse(Harness::ClaudeCode, CLAUDE, 0);
        assert_eq!(parsed.title, "T3code service restart");
        assert_eq!(
            summary(&parsed),
            [
                row(
                    "u1",
                    "u1",
                    "user",
                    "bring the service back up\nit went down"
                ),
                row(
                    "toolu_1",
                    "u1",
                    "tool",
                    "Bash {\"command\":\"systemctl status t3\"} -> inactive (dead) (failed)"
                ),
                row("a2", "u1", "agent", "It was stopped; I restarted it."),
                row("u2", "u2", "user", "thanks"),
            ]
        );
        let Detail::Agent(answer) = parsed.events[2].detail.as_ref().unwrap() else {
            panic!()
        };
        assert_eq!(answer.model, "claude-opus-5-5");
        let Detail::Tool(call) = parsed.events[1].detail.as_ref().unwrap() else {
            panic!()
        };
        // The call began when the agent asked for it and ended with its result.
        assert_eq!(call.started_at.seconds + 4, parsed.events[1].at.seconds);
    }

    #[test]
    fn a_later_read_returns_only_new_lines_but_still_pairs_and_names_the_turn() {
        // The cursor sits after the tool_use line: its result and the answer are new, and they
        // still belong to the turn the first prompt began.
        let from = CLAUDE.find(r#"{"type":"assistant","uuid":"side""#).unwrap();
        let parsed = parse(Harness::ClaudeCode, CLAUDE, from);
        assert_eq!(
            summary(&parsed)[..2],
            [
                row(
                    "toolu_1",
                    "u1",
                    "tool",
                    "Bash {\"command\":\"systemctl status t3\"} -> inactive (dead) (failed)"
                ),
                row("a2", "u1", "agent", "It was stopped; I restarted it."),
            ]
        );
        assert!(
            parse(Harness::ClaudeCode, CLAUDE, CLAUDE.len())
                .events
                .is_empty()
        );
    }

    #[test]
    fn codex_items_of_a_turn() {
        let rollout = concat!(
            r#"{"timestamp":"2026-11-14T22:34:25.176Z","type":"session_meta","payload":{"id":"s"}}"#,
            "\n",
            r#"{"timestamp":"2026-11-14T22:34:25.377Z","type":"turn_context","payload":{"turn_id":"t","model":"gpt-6.1-sol"}}"#,
            "\n",
            r#"{"timestamp":"2026-11-14T22:34:25.483Z","type":"event_msg","payload":{"type":"item_completed","item":{"type":"UserMessage","id":"um1","content":[{"type":"text","text":"is this machine fast enough?"}]},"started_at_ms":1794695665483,"completed_at_ms":1794695665483}}"#,
            "\n",
            r#"{"timestamp":"2026-11-14T22:34:26Z","type":"event_msg","payload":{"type":"item_completed","item":{"type":"Reasoning","id":"rs1","summary_text":[]}}}"#,
            "\n",
            r#"{"timestamp":"2026-11-14T22:34:31Z","type":"event_msg","payload":{"type":"item_completed","item":{"type":"CommandExecution","id":"exec-1","command":["/bin/bash","-lc","lscpu | head -2"],"status":"completed","exit_code":0,"aggregated_output":"x86_64"},"started_at_ms":1794695669000,"completed_at_ms":1794695671000}}"#,
            "\n",
            r#"{"timestamp":"2026-11-14T22:34:32Z","type":"event_msg","payload":{"type":"item_completed","item":{"type":"McpToolCall","id":"exec-2","server":"t3-code","tool":"t3_thread_read","arguments":{"limit":4},"status":"failed","result":null}}}"#,
            "\n",
            r#"{"timestamp":"2026-11-14T22:34:33Z","type":"event_msg","payload":{"type":"item_completed","item":{"type":"Extension","kind":"web.search","id":"exec-3","query":"evo x3 price"}}}"#,
            "\n",
            r#"{"timestamp":"2026-11-14T22:34:46Z","type":"event_msg","payload":{"type":"item_completed","item":{"type":"AgentMessage","id":"msg_1","content":[{"type":"Text","text":"Fast enough, but pricey."}],"phase":"final_answer"}}}"#,
            "\n",
            r#"{"timestamp":"2026-11-14T22:34:46.249Z","type":"event_msg","payload":{"type":"task_complete","last_agent_message":"Fast enough, but pricey."}}"#,
            "\n",
        );
        let parsed = parse(Harness::Codex, rollout, 0);
        assert_eq!(parsed.title, "is this machine fast enough?");
        assert_eq!(
            summary(&parsed),
            [
                row("um1", "um1", "user", "is this machine fast enough?"),
                row("exec-1", "um1", "tool", "shell lscpu | head -2 -> x86_64"),
                row(
                    "exec-2",
                    "um1",
                    "tool",
                    "t3-code.t3_thread_read {\"limit\":4} ->  (failed)"
                ),
                row("exec-3", "um1", "tool", "web.search evo x3 price -> "),
                row("msg_1", "um1", "agent", "Fast enough, but pricey."),
            ]
        );
        let Detail::Agent(answer) = parsed.events[4].detail.as_ref().unwrap() else {
            panic!()
        };
        assert_eq!(answer.model, "gpt-6.1-sol");
        assert_eq!(parsed.events[1].at.seconds, 1794695671);
    }

    #[test]
    fn oversized_tool_output_is_cut_on_a_character_boundary() {
        let cut = cut(&"é".repeat(MAX_TOOL), MAX_TOOL);
        assert!(cut.len() <= MAX_TOOL + "\n[truncated]".len());
        assert!(cut.ends_with("[truncated]"));
    }
}
