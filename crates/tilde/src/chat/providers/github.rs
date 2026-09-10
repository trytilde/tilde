//! GitHub issue/PR comments and review replies use the original chat provider's distinct endpoints.
use super::*;
use crate::proto::tilde::types::v1 as types;
pub struct Github;
impl Adapter for Github {
    fn webhook<'a>(
        &'a self,
        a: &'a Access,
        h: &'a http::HeaderMap,
        body: &'a [u8],
    ) -> BoxFuture<'a, ToolResult<ingress::Webhook>> {
        Box::pin(async move {
            use ingress::*;
            sha256_header(a, h, body, "webhook_secret")?;
            let p = payload(body)?;
            if p.pointer("/installation/id")
                .map(Value::to_string)
                .as_deref()
                != Some(a.secret("installation_id")?)
            {
                return Err(ConnectError::permission_denied(
                    "GitHub installation mismatch",
                ));
            }
            let event = header(h, "x-github-event")?;
            if !["issue_comment", "pull_request_review_comment"].contains(&event)
                || at(&p, "/action") != Some("created")
                || at(&p, "/sender/type") == Some("Bot")
            {
                return Ok(Webhook::Messages(vec![]));
            }
            let repo = at(&p, "/repository/full_name")
                .ok_or_else(|| ConnectError::invalid_argument("Missing repository"))?;
            let number = p
                .pointer("/issue/number")
                .or_else(|| p.pointer("/pull_request/number"))
                .filter(|v| v.is_number())
                .ok_or_else(|| ConnectError::invalid_argument("Missing issue number"))?
                .to_string();
            let sender = p
                .pointer("/sender/id")
                .filter(|v| v.is_number())
                .ok_or_else(|| ConnectError::invalid_argument("Missing sender"))?
                .to_string();
            let comment = p
                .pointer("/comment/id")
                .filter(|v| v.is_number())
                .ok_or_else(|| ConnectError::invalid_argument("Missing comment"))?
                .to_string();
            Ok(Webhook::Messages(vec![IncomingMessage {
                subject: None,
                kind: IncomingKind::Message,
                attachments: vec![],
                event_id: header(h, "x-github-delivery")?.into(),
                message_id: comment,
                thread_id: format!("{repo}#{number}"),
                sender_id: sender,
                sender_name: at(&p, "/sender/login").unwrap_or("GitHub user").into(),
                text: at(&p, "/comment/body").unwrap_or("").into(),
                format: "markdown",
                reply_to: None,
            }]))
        })
    }

    fn tools(&self) -> Vec<types::ToolDefinition> {
        vec![
            tool(
                "sendMessage",
                "Comment on a GitHub issue or pull request using Markdown.",
                json!({"owner":string(),"repository":string(),"number":{"type":"integer","minimum":1},"text":text()}),
                &["owner", "repository", "number", "text"],
                true,
            ),
            tool(
                "replyToReview",
                "Reply within a pull-request review comment thread using Markdown.",
                json!({"owner":string(),"repository":string(),"number":{"type":"integer","minimum":1},"commentId":{"type":"integer","minimum":1},"text":text()}),
                &["owner", "repository", "number", "commentId", "text"],
                true,
            ),
            tool(
                "reactToMessage",
                "React to an issue or pull-request conversation comment.",
                json!({"owner":string(),"repository":string(),"commentId":{"type":"integer","minimum":1},"reaction":{"enum":["+1","-1","laugh","confused","heart","hooray","rocket","eyes"]}}),
                &["owner", "repository", "commentId", "reaction"],
                false,
            ),
        ]
    }
    fn invoke<'a>(
        &'a self,
        a: &'a Access,
        c: &'a Context,
        name: &'a str,
        v: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            let owner = required(&v, "owner")?;
            let repo = required(&v, "repository")?;
            let number = v["number"].to_string();
            let comment = v["commentId"].to_string();
            let (segments, body) = match name {
                "sendMessage" => (
                    vec!["repos", owner, repo, "issues", &number, "comments"],
                    json!({"body":required(&v,"text")?}),
                ),
                "replyToReview" => (
                    vec![
                        "repos", owner, repo, "pulls", &number, "comments", &comment, "replies",
                    ],
                    json!({"body":required(&v,"text")?}),
                ),
                "reactToMessage" => (
                    vec![
                        "repos",
                        owner,
                        repo,
                        "issues",
                        "comments",
                        &comment,
                        "reactions",
                    ],
                    json!({"content":required(&v,"reaction")?}),
                ),
                _ => return Err(ConnectError::not_found("Unknown GitHub tool")),
            };
            let response = a
                .json(
                    a.post(
                        a.url("github_api", "https://api.github.com", &segments)?,
                        "access_token",
                    )?
                    .header("Accept", "application/vnd.github+json")
                    .header("X-GitHub-Api-Version", "2022-11-28")
                    .json(&body),
                )
                .await?;
            let external = response
                .get("id")
                .filter(|id| id.is_number())
                .ok_or_else(|| ConnectError::internal("GitHub omitted operation ID"))?
                .to_string();
            if name == "reactToMessage" {
                Ok(json!({"reactionId":external}))
            } else {
                sent(
                    c,
                    a,
                    required(&v, "text")?,
                    "markdown",
                    &format!("{owner}/{repo}#{number}"),
                    &external,
                    None,
                )
                .await
            }
        })
    }
}
