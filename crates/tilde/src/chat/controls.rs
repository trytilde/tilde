//! Execution-owned control streams. Terminal credentials can only observe/ack
//! their own stop; this service never grants revoked application capabilities.
use super::{Chat, ChatError, Result, id};
use crate::proto::tilde::types::v1 as types;
use crate::{iam::tokens::Claims, proto::tilde::runtime::v1 as wire};
use connectrpc::{RequestContext, Response, ServiceRequest, ServiceResult, ServiceStream};
use futures::{Stream, StreamExt};
use secrecy::{ExposeSecret, SecretString};
use serde_json::json;
use std::{pin::Pin, sync::Arc, time::Duration};
use uuid::Uuid;
use wire::{InvocationCommandKind as Kind, WatchCommandsResponse as Command};
type Changes = Pin<Box<dyn Stream<Item = Result<()>> + Send>>;
fn terminal(invocation: Uuid, kind: Kind) -> Command {
    Command {
        id: Uuid::new_v5(
            &invocation,
            if kind == Kind::Suspend {
                b"suspend"
            } else {
                b"stop"
            },
        )
        .to_string(),
        kind: kind.into(),
        ..Default::default()
    }
}
impl Chat {
    pub async fn suspend_invocation(&self, invocation: Uuid) -> Result<()> {
        if let Some(local) = self.local() {
            let v = local.invocation(invocation).await?;
            if v.status != "running" {
                return Err(ChatError::Conflict);
            }
            let scope = super::Scope {
                id: invocation,
                run_id: id(&v.run_id)?,
                thread_id: id(&v.thread_id)?,
                agent_id: local.agent_id,
                participant_id: id(&v.participant_id)?,
                capabilities: Default::default(),
            };
            let _guard = local.mutation.lock().await;
            if local.invocation(invocation).await?.status != "running" {
                return Err(ChatError::Conflict);
            }
            let mut run = local.run(scope.run_id).await?;
            run.status = "suspending".into();
            return local
                .commit(
                    scope.thread_id,
                    vec![
                        crate::deployment::corrosion::client::statement(
                            "UPDATE runs SET status=?,payload=? WHERE id=?",
                            vec![
                                json!(run.status),
                                json!(local.seal(scope.run_id, "run", &run)?),
                                json!(run.id),
                            ],
                        ),
                        local.event(scope.thread_id, "run.updated", run.into())?,
                    ],
                )
                .await;
        }
        let row = sqlx::query_file!("../../queries/chat/invocation_endpoint.sql", invocation)
            .fetch_one(self.pg()?)
            .await?;
        if row.status != "running" {
            return Err(ChatError::Conflict);
        }
        let mut tx = self.pg()?.begin().await?;
        sqlx::query_file!("../../queries/chat/thread_lock.sql", row.thread_id)
            .fetch_one(&mut *tx)
            .await?;
        if sqlx::query_file!(
            "../../queries/chat/controls/suspend.sql",
            row.run_id,
            invocation
        )
        .execute(&mut *tx)
        .await?
        .rows_affected()
            == 0
        {
            return Err(ChatError::Conflict);
        }
        super::activity(
            &mut tx,
            row.thread_id,
            "invocation.suspend_requested",
            invocation,
            "",
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
    async fn control_commands(&self, claims: &Claims, token: &str) -> Result<Vec<Command>> {
        match self.scope(token).await {
            Ok(_) => {}
            Err(ChatError::Denied | ChatError::NotFound) => {
                return Ok(vec![terminal(claims.invocation_id, Kind::Stop)]);
            }
            Err(error) => return Err(error),
        }
        let mut commands = vec![];
        if let Some(local) = self.local() {
            if local.run(claims.run_id).await?.status == "suspending" {
                return Ok(vec![terminal(claims.invocation_id, Kind::Suspend)]);
            }
            for row in local.client.query::<crate::deployment::runtime::execution::CommandRow>("SELECT * FROM commands WHERE thread_id=? AND kind='steer' AND acked_at IS NULL AND finished_at IS NULL ORDER BY created_at,id",vec![json!(claims.thread_id)]).await? {
                let command:types::AgentCommand=local.open(id(&row.id)?,"command",&row.payload)?;
                if let Some(types::agent_command::Action::Steer(input))=command.action && input.invocation_id==claims.invocation_id.to_string() {
                    commands.push(Command{id:command.id,kind:Kind::Steer.into(),input_id:input.input_id,text:input.text,message:input.message,..Default::default()});
                }
            }
        } else {
            let state = sqlx::query_file!(
                "../../queries/chat/controls/state.sql",
                claims.invocation_id,
                claims.sub,
                claims.thread_id,
                claims.run_id
            )
            .fetch_one(self.pg()?)
            .await?;
            if state.run_status == "suspending" {
                return Ok(vec![terminal(claims.invocation_id, Kind::Suspend)]);
            }
            for input in sqlx::query_file!(
                "../../queries/chat/inputs_pending.sql",
                claims.invocation_id
            )
            .fetch_all(self.pg()?)
            .await?
            {
                commands.push(Command {
                    id: input.id.to_string(),
                    kind: Kind::Steer.into(),
                    input_id: input.id.to_string(),
                    text: input.text,
                    message: self
                        .steering_message(claims.thread_id, input.id)
                        .await?
                        .into(),
                    ..Default::default()
                });
            }
        }
        Ok(commands)
    }
    async fn control_changes(&self, claims: &Claims) -> Result<Changes> {
        if let Some(local) = self.local() {
            let queries = [
                (
                    "SELECT * FROM invocations WHERE id=?",
                    json!(claims.invocation_id),
                ),
                (
                    "SELECT * FROM commands WHERE thread_id=?",
                    json!(claims.thread_id),
                ),
                (
                    "SELECT * FROM assignments WHERE thread_id=?",
                    json!(claims.thread_id),
                ),
                (
                    "SELECT * FROM configuration WHERE agent_id=?",
                    json!(claims.sub),
                ),
                ("SELECT * FROM runs WHERE id=?", json!(claims.run_id)),
            ];
            let mut streams: Vec<Changes> = vec![];
            for (sql, value) in queries {
                streams.push(Box::pin(
                    local
                        .client
                        .subscribe::<serde_json::Value>(sql, vec![value])
                        .await?
                        .map(|r| r.map(|_| ())),
                ));
            }
            Ok(Box::pin(futures::stream::select_all(streams)))
        } else {
            let notifications = self.control_notifications.clone();
            let mut changed = notifications
                .subscribe(self.pg()?, "tilde_invocation_control")
                .await?;
            Ok(Box::pin(
                async_stream::try_stream! {let _notifications=notifications;while changed.changed().await.is_ok(){changed.borrow_and_update();yield ();}},
            ))
        }
    }
    pub async fn acknowledge_control(&self, token: &str, key: Uuid) -> Result<()> {
        let claims = self.tokens.verify_trace(token).await?;
        let commands = self.control_commands(&claims, token).await?;
        let command = commands.iter().find(|c| c.id == key.to_string());
        if let Some(local) = self.local() {
            let acknowledged = local
                .client
                .query::<crate::deployment::runtime::IdRow>(
                    "SELECT id FROM control_receipts WHERE id=? AND invocation_id=?",
                    vec![json!(key), json!(claims.invocation_id)],
                )
                .await?;
            if !acknowledged.is_empty() {
                return Ok(());
            }
            let _command = command.ok_or(ChatError::Denied)?;
            local.commit(claims.thread_id,vec![crate::deployment::corrosion::client::statement("INSERT INTO control_receipts(id,thread_id,invocation_id,acknowledged_at) VALUES(?,?,?,?) ON CONFLICT(id) DO NOTHING",vec![json!(key),json!(claims.thread_id),json!(claims.invocation_id),json!(chrono::Utc::now().timestamp_millis())]),crate::deployment::corrosion::client::statement("UPDATE commands SET acked_at=?,finished_at=? WHERE id=? AND thread_id=? AND kind='steer'",vec![json!(chrono::Utc::now().timestamp_millis()),json!(chrono::Utc::now().timestamp_millis()),json!(key),json!(claims.thread_id)]),local.event(claims.thread_id,"command.acknowledged",types::Activity{kind:"command.acknowledged".into(),entity_id:key.to_string(),invocation_id:claims.invocation_id.to_string(),..Default::default()}.into())?]).await?;
        } else {
            let mut tx = self.pg()?.begin().await?;
            if command.is_none() {
                let exists = sqlx::query_file!(
                    "../../queries/chat/controls/receipt.sql",
                    claims.invocation_id,
                    key
                )
                .fetch_one(&mut *tx)
                .await?
                .exists;
                return if exists {
                    Ok(())
                } else {
                    Err(ChatError::Denied)
                };
            }
            let command = command.ok_or(ChatError::Denied)?;
            if command.kind.as_known() == Some(Kind::Steer) {
                sqlx::query_file!(
                    "../../queries/chat/input_accept.sql",
                    claims.invocation_id,
                    key
                )
                .execute(&mut *tx)
                .await?;
            }
            if sqlx::query_file!(
                "../../queries/chat/controls/ack.sql",
                claims.invocation_id,
                key
            )
            .fetch_optional(&mut *tx)
            .await?
            .is_some()
            {
                super::activity(&mut tx, claims.thread_id, "command.acknowledged", key, "").await?;
            }
            tx.commit().await?;
        }
        Ok(())
    }
}
struct Rpc(Chat);
pub fn router(chat: Chat) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Rpc(chat))),
        256 * 1024,
    )
}
impl crate::services::tilde::runtime::v1::InvocationControlService for Rpc {
    async fn watch_commands(
        &self,
        ctx: RequestContext,
        _: ServiceRequest<'_, wire::WatchCommandsRequest>,
    ) -> ServiceResult<ServiceStream<impl connectrpc::Encodable<Command> + Send + use<>>> {
        let token = SecretString::from(super::rpc::capability(&ctx)?);
        let claims = self.0.tokens.verify_trace(token.expose_secret()).await?;
        let mut changed = self.0.control_changes(&claims).await?;
        let chat = self.0.clone();
        Response::stream_ok(async_stream::try_stream! {
            let mut seen=std::collections::BTreeSet::new();
            let mut ready=false;
            let deadline=tokio::time::sleep(Duration::from_secs((claims.exp-chrono::Utc::now().timestamp()).max(1) as u64));tokio::pin!(deadline);
            loop {
                let commands=chat.control_commands(&claims,token.expose_secret()).await?;
                if !commands.iter().any(|c|matches!(c.kind.as_known(),Some(Kind::Stop|Kind::Suspend))) && !ready {ready=true;yield Command{kind:Kind::Ready.into(),..Default::default()};}
                seen.retain(|id|commands.iter().any(|c|&c.id==id));
                for command in commands {let terminal=matches!(command.kind.as_known(),Some(Kind::Stop|Kind::Suspend));if seen.insert(command.id.clone()){yield command;}
                    if terminal{return;}}
                let update=tokio::select!{_= &mut deadline=>None,value=changed.next()=>value};
                match update{Some(value)=>value?,None=>break}
            }
        })
    }
    async fn acknowledge_command<'a>(
        &'a self,
        ctx: RequestContext,
        r: ServiceRequest<'_, wire::AcknowledgeCommandRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::AcknowledgeCommandResponse> + Send + use<'a>>
    {
        self.0
            .acknowledge_control(super::rpc::capability(&ctx)?, id(r.id)?)
            .await?;
        Response::ok(wire::AcknowledgeCommandResponse::default())
    }
}
