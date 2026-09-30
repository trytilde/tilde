//! Slack Web API tools for messages, channels, reactions, pins, files, users and user groups.
//! Every tool is one call to `https://slack.com/api/<method>` with the connection's bot token.
//!
//! The `slack_app` connection authorizes bot scopes only, so it holds no user token. Methods
//! Slack allows only for users (search, reminders, Do Not Disturb, setting a status or presence,
//! Enterprise Grid admin) are listed but report that they need one. Workspaces installed before
//! the tool scopes were added answer `missing_scope` until the app is reinstalled.
use super::{ToolProvider, rest};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use serde_json::{Map, Value, json};

pub struct Slack;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Call {
    /// Query parameters on a GET.
    Get,
    /// A JSON POST.
    Json,
    /// A form-encoded POST, for methods that do not accept JSON.
    Form,
}
struct Tool {
    name: &'static str,
    summary: &'static str,
    description: &'static str,
    method: &'static str,
    call: Call,
    /// Slack permits the method only with a user token.
    user: bool,
    /// Space-separated input fields; `!` marks required ones.
    fields: &'static str,
    destructive: bool,
}
const fn tool(
    name: &'static str,
    summary: &'static str,
    description: &'static str,
    method: &'static str,
    call: Call,
    fields: &'static str,
) -> Tool {
    Tool {
        name,
        summary,
        description,
        method,
        call,
        user: false,
        fields,
        destructive: false,
    }
}
const fn user(tool: Tool) -> Tool {
    Tool { user: true, ..tool }
}
const fn destructive(tool: Tool) -> Tool {
    Tool {
        destructive: true,
        ..tool
    }
}

use Call::{Form, Get, Json};
#[rustfmt::skip]
const TOOLS: &[Tool] = &[
    tool("slack_send_message", "Sent a Slack message", "Send a message to a Slack channel.", "chat.postMessage", Json, "!channel_id !text thread_ts"),
    tool("slack_reply_in_thread", "Replied in a Slack thread", "Reply to an existing Slack thread.", "chat.postMessage", Json, "!channel_id !thread_ts !text"),
    tool("slack_list_channels", "Listed Slack channels", "List Slack conversations visible to the bot.", "conversations.list", Get, "include_private cursor limit"),
    tool("slack_add_reaction", "Reacted in Slack", "Add a reaction to a Slack message.", "reactions.add", Json, "!channel_id !timestamp !reaction"),
    destructive(tool("slack_archive_channel", "Archived a Slack channel", "Archive a Slack channel.", "conversations.archive", Json, "!channel_id")),
    tool("slack_get_channel_info", "Read Slack channel info", "Retrieve conversation details including topic, purpose, and membership state.", "conversations.info", Get, "!channel_id include_locale include_num_members"),
    tool("slack_invite_to_channel", "Invited users to a Slack channel", "Invite users to a Slack channel.", "conversations.invite", Json, "!channel_id !user_ids"),
    tool("slack_set_channel_purpose", "Set a Slack channel purpose", "Update a channel's purpose text for a conversation.", "conversations.setPurpose", Json, "!channel_id !purpose"),
    tool("slack_set_channel_topic", "Set a Slack channel topic", "Set the topic of a channel.", "conversations.setTopic", Json, "!channel_id !topic"),
    tool("slack_unarchive_channel", "Unarchived a Slack channel", "Restore an archived conversation so members can use it again.", "conversations.unarchive", Json, "!channel_id"),
    user(tool("slack_set_channel_workspaces", "Set Slack channel workspaces", "Set the workspaces in an Enterprise Grid org that connect to a channel. Requires a Slack user token with admin scopes.", "admin.conversations.setTeams", Json, "!channel_id team_id target_team_ids org_channel")),
    tool("slack_create_channel", "Created a Slack channel", "Create a new public or private Slack channel by name.", "conversations.create", Json, "!name is_private team_id"),
    tool("slack_get_conversation_history", "Read Slack history", "Fetch paginated message history for a conversation within optional time bounds.", "conversations.history", Get, "!channel_id cursor limit latest oldest inclusive"),
    tool("slack_join_channel", "Joined a Slack channel", "Join a public channel and return its conversation details.", "conversations.join", Json, "!channel_id"),
    tool("slack_leave_channel", "Left a Slack channel", "Leave a channel.", "conversations.leave", Json, "!channel_id"),
    tool("slack_get_channel_members", "Listed Slack channel members", "List members in a Slack channel.", "conversations.members", Get, "!channel_id cursor limit"),
    tool("slack_list_channels_v2", "Listed Slack conversations", "List Slack conversations with optional type filters and cursor pagination.", "conversations.list", Get, "types exclude_archived cursor limit team_id"),
    tool("slack_mark_as_read", "Marked a Slack conversation read", "Move a conversation's read cursor to a specific message timestamp.", "conversations.mark", Json, "!channel_id !timestamp"),
    tool("slack_open_dm", "Opened a Slack DM", "Open a direct or multi-person DM for specified users.", "conversations.open", Json, "!user_ids return_im"),
    destructive(tool("slack_remove_from_channel", "Removed a user from a Slack channel", "Remove a user from a channel.", "conversations.kick", Json, "!channel_id !user_id")),
    tool("slack_rename_channel", "Renamed a Slack channel", "Rename a Slack channel.", "conversations.rename", Json, "!channel_id !name"),
    user(tool("slack_get_dnd_info", "Read Slack Do Not Disturb", "Get a user's Do Not Disturb status and next scheduled DND window. Requires a Slack user token.", "dnd.info", Get, "user_id")),
    tool("slack_list_custom_emoji", "Listed Slack custom emoji", "List workspace custom emoji mappings, including alias-based emoji entries.", "emoji.list", Get, ""),
    tool("slack_get_upload_url", "Prepared a Slack upload", "Generate an external upload URL and file ID for Slack uploads.", "files.getUploadURLExternal", Form, "!filename !length alt_txt snippet_type"),
    tool("slack_list_files", "Listed Slack files", "List files shared in the workspace.", "files.list", Get, "channel_id user_id ts_from ts_to types count page show_files_hidden_by_limit"),
    user(tool("slack_search_files", "Searched Slack files", "Search workspace files with pagination. Requires a Slack user token.", "search.files", Get, "!query sort sort_dir count page highlight")),
    destructive(tool("slack_delete_message", "Deleted a Slack message", "Delete a message from a channel.", "chat.delete", Json, "!channel_id !timestamp")),
    destructive(tool("slack_delete_scheduled_message", "Cancelled a scheduled Slack message", "Cancel a scheduled message.", "chat.deleteScheduledMessage", Json, "!channel_id !scheduled_message_id")),
    user(tool("slack_search_messages", "Searched Slack messages", "Search for messages matching a query. Requires a Slack user token.", "search.messages", Get, "!query sort sort_dir count page highlight")),
    tool("slack_get_message_permalink", "Linked a Slack message", "Get a permanent URL for a message.", "chat.getPermalink", Get, "!channel_id !timestamp"),
    tool("slack_get_thread_replies", "Read a Slack thread", "Fetch paginated thread replies and parent message for a conversation thread.", "conversations.replies", Get, "!channel_id !thread_ts cursor limit latest oldest inclusive"),
    tool("slack_list_scheduled_messages", "Listed scheduled Slack messages", "List pending scheduled messages.", "chat.scheduledMessages.list", Get, "channel_id latest oldest limit cursor"),
    tool("slack_schedule_message", "Scheduled a Slack message", "Schedule a Slack message to a channel or thread.", "chat.scheduleMessage", Json, "!channel_id !text !post_at thread_ts"),
    tool("slack_post_message", "Posted a Slack message", "Post a message to a channel, DM, or thread.", "chat.postMessage", Json, "!channel_id !text thread_ts"),
    tool("slack_update_message", "Edited a Slack message", "Edit an existing message in a Slack channel.", "chat.update", Json, "!channel_id !timestamp !text"),
    tool("slack_send_ephemeral_message", "Sent an ephemeral Slack message", "Send a message visible only to one user in a channel.", "chat.postEphemeral", Json, "!channel_id !user_id !text thread_ts"),
    tool("slack_list_pins", "Listed Slack pins", "List all items pinned in a specific channel.", "pins.list", Get, "!channel_id"),
    tool("slack_pin_message", "Pinned a Slack message", "Pin a specific message in a channel.", "pins.add", Json, "!channel_id !timestamp"),
    tool("slack_unpin_message", "Unpinned a Slack message", "Remove a pinned message from a channel.", "pins.remove", Json, "!channel_id !timestamp"),
    tool("slack_get_reactions", "Read Slack reactions", "Retrieve all reactions attached to a specific Slack message.", "reactions.get", Get, "!channel_id !timestamp full"),
    tool("slack_list_user_reactions", "Listed Slack user reactions", "List items the user reacted to with cursor-based pagination.", "reactions.list", Get, "user_id full count page cursor"),
    tool("slack_remove_reaction", "Removed a Slack reaction", "Remove an emoji reaction from a specific Slack message.", "reactions.remove", Json, "!channel_id !timestamp !reaction"),
    user(tool("slack_create_reminder", "Created a Slack reminder", "Create a reminder for a user. Requires a Slack user token.", "reminders.add", Json, "!text !time user_id")),
    tool("slack_get_team_info", "Read Slack workspace info", "Retrieve workspace details such as name, domain, and icon.", "team.info", Get, "team_id"),
    tool("slack_list_user_group_members", "Listed Slack user group members", "List member user IDs for a specific Slack user group.", "usergroups.users.list", Get, "!usergroup_id include_disabled team_id"),
    tool("slack_list_user_groups", "Listed Slack user groups", "List workspace user groups with optional disabled and membership counts.", "usergroups.list", Get, "include_users include_disabled include_count team_id"),
    tool("slack_lookup_user_by_email", "Looked up a Slack user", "Look up a user by email address.", "users.lookupByEmail", Get, "!email"),
    tool("slack_get_user_info", "Read a Slack user", "Retrieve a user's account details, including profile and avatar fields.", "users.info", Get, "!user_id include_locale"),
    tool("slack_get_user_presence", "Read Slack presence", "Check if a user is online or away.", "users.getPresence", Get, "user_id"),
    tool("slack_get_user_profile", "Read a Slack profile", "Retrieve a user's detailed profile, status, and custom fields.", "users.profile.get", Get, "user_id include_labels"),
    tool("slack_list_users", "Listed Slack users", "List all users in the workspace.", "users.list", Get, "cursor limit include_locale team_id"),
    user(tool("slack_set_status", "Set a Slack status", "Set a user's status. Requires a Slack user token.", "users.profile.set", Json, "user_id !status_text !status_emoji status_expiration")),
    user(tool("slack_set_user_presence", "Set Slack presence", "Set a user's presence to online or away. Requires a Slack user token.", "users.setPresence", Json, "!presence")),
];

fn property(name: &str) -> Value {
    let text = |description: &str| json!({"type":"string","description":description});
    let flag = || json!({"type":"boolean"});
    let count = |max: u32| json!({"type":"integer","minimum":1,"maximum":max});
    let ids = |description: &str| json!({"type":"array","items":{"type":"string"},"minItems":1,"description":description});
    match name {
        "channel_id" => text("Conversation ID, e.g. C0123456789."),
        "user_id" => text("User ID, e.g. U0123456789."),
        "user_ids" => ids("User IDs."),
        "target_team_ids" => ids("Workspace IDs."),
        "types" => ids(
            "Types to include: public_channel, private_channel, mpim, im for conversations; spaces, snippets, images, pdfs, etc. for files.",
        ),
        "timestamp" => text("Message timestamp (ts)."),
        "thread_ts" => text("Timestamp of the thread's parent message."),
        "reaction" => text("Emoji name, e.g. thumbsup."),
        "limit" => count(1000),
        "count" => count(1000),
        "page" | "length" => json!({"type":"integer","minimum":1}),
        "post_at" | "status_expiration" => {
            json!({"type":"integer","minimum":0,"description":"Unix time in seconds."})
        }
        "time" => text("When to remind: Unix time, or natural language such as \"in 15 minutes\"."),
        "latest" | "oldest" | "ts_from" | "ts_to" => text("Timestamp bound."),
        "query" => text("Slack search query."),
        "sort" => json!({"enum":["score","timestamp"]}),
        "sort_dir" => json!({"enum":["asc","desc"]}),
        "presence" => json!({"enum":["auto","away"]}),
        "email" => json!({"type":"string","format":"email"}),
        "include_private"
        | "include_locale"
        | "include_num_members"
        | "org_channel"
        | "is_private"
        | "inclusive"
        | "exclude_archived"
        | "return_im"
        | "show_files_hidden_by_limit"
        | "highlight"
        | "full"
        | "include_disabled"
        | "include_users"
        | "include_count"
        | "include_labels" => flag(),
        _ => json!({"type":"string"}),
    }
}

fn definition(tool: &Tool) -> types::ToolDefinition {
    let mut properties = Map::new();
    let mut required = vec![];
    for field in tool.fields.split_whitespace() {
        let name = field.trim_start_matches('!');
        if field.starts_with('!') {
            required.push(name);
        }
        properties.insert(name.to_owned(), property(name));
    }
    rest::definition(
        "slack",
        tool.name,
        tool.summary,
        tool.description,
        json!({"type":"object","properties":properties,"required":required,"additionalProperties":false}),
        rest::Hints {
            read_only: tool.call == Get,
            destructive: tool.destructive,
        },
    )
}

/// The tool's input as Slack's parameters: Slack names, comma-joined lists and defaults.
fn params(tool: &Tool, input: Value) -> Map<String, Value> {
    let Value::Object(input) = input else {
        return Map::new();
    };
    let mut out = Map::new();
    for (key, value) in input {
        let key = match (tool.method, key.as_str()) {
            ("admin.conversations.setTeams", _) => key,
            (_, "channel_id") => "channel".into(),
            (_, "user_id") => "user".into(),
            (_, "user_ids") => "users".into(),
            (_, "usergroup_id") => "usergroup".into(),
            ("team.info", "team_id") => "team".into(),
            ("conversations.replies", "thread_ts") => "ts".into(),
            ("chat.delete" | "chat.update" | "conversations.mark", "timestamp") => "ts".into(),
            ("chat.getPermalink", "timestamp") => "message_ts".into(),
            (_, "reaction") => "name".into(),
            _ => key,
        };
        let value = match value {
            Value::Array(items) => Value::String(
                items
                    .iter()
                    .map(|item| item.as_str().map(str::to_owned).unwrap_or(item.to_string()))
                    .collect::<Vec<_>>()
                    .join(","),
            ),
            Value::String(text) if key == "name" && tool.method.starts_with("reactions.") => {
                Value::String(text.trim_matches(':').to_owned())
            }
            other => other,
        };
        out.insert(key, value);
    }
    match tool.name {
        "slack_list_channels" => {
            let private = out.remove("include_private").and_then(|v| v.as_bool());
            out.insert(
                "types".into(),
                if private == Some(true) {
                    "public_channel,private_channel"
                } else {
                    "public_channel"
                }
                .into(),
            );
            out.entry("limit").or_insert(100.into());
        }
        "slack_list_channels_v2" => {
            out.entry("types")
                .or_insert("public_channel,private_channel".into());
            out.entry("exclude_archived").or_insert(true.into());
            out.entry("limit").or_insert(100.into());
        }
        _ => {}
    }
    out
}
fn scalar(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

impl ToolProvider for Slack {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        TOOLS.iter().map(definition).collect()
    }
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        _call_id: uuid::Uuid,
        name: &'a str,
        input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            let tool = TOOLS
                .iter()
                .find(|tool| tool.name == name)
                .ok_or_else(|| ConnectError::not_found("Unsupported Slack tool"))?;
            if tool.user {
                return Err(ConnectError::failed_precondition(format!(
                    "Slack allows {} only with a user token, and this Slack app connection holds a bot token",
                    tool.method
                )));
            }
            let params = params(tool, input);
            let url = access.url("slack_api", "https://slack.com/api", &[tool.method])?;
            let client = &access.http.client;
            let request = match tool.call {
                Get => {
                    let query: Vec<(String, String)> =
                        params.iter().map(|(k, v)| (k.clone(), scalar(v))).collect();
                    client.get(url).query(&query)
                }
                Json => client.post(url).json(&params),
                Form => {
                    let form: Vec<(String, String)> =
                        params.iter().map(|(k, v)| (k.clone(), scalar(v))).collect();
                    client.post(url).form(&form)
                }
            };
            let response = rest::send(request.bearer_auth(access.secret("access_token")?)).await?;
            let data = &response["data"];
            if data["ok"] != true {
                let error = data["error"].as_str().unwrap_or("unknown_error");
                return Err(ConnectError::unknown(match data["needed"].as_str() {
                    Some(needed) => format!("Slack returned {error} (needs the {needed} scope)"),
                    None => format!("Slack returned {error}"),
                }));
            }
            Ok(response["data"].clone())
        })
    }
}
