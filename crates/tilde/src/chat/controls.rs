//! Execution-owned control streams. Terminal credentials can only observe/ack
//! their own stop. Control delivery is independent of a still-valid action token.
use super::{Chat, ChatError, Result, id};
use crate::{iam::tokens::Claims, proto::tilde::runtime::v1 as wire};
use connectrpc::{RequestContext, Response, ServiceRequest, ServiceResult, ServiceStream};
use futures::{Stream, StreamExt};
use secrecy::{ExposeSecret, SecretString};
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
            return local.suspend_invocation(invocation).await;
        }
        let row =
            crate::chat::db::invocation_endpoint_one(&self.pg()?.get().await?, invocation).await?;
        if let Some(()) = self
            .forward_control(
                row.agent_id,
                row.thread_id,
                "SuspendInvocation",
                &crate::proto::tilde::provider::tilde::v1::SuspendInvocationRequest {
                    invocation_id: invocation.to_string(),
                    ..Default::default()
                },
            )
            .await?
        {
            return Ok(());
        }
        if row.status != "running" {
            return Err(ChatError::Conflict);
        }
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        crate::chat::db::thread_lock_one(&tx, row.thread_id).await?;
        if crate::chat::db::controls_suspend_execute(&tx, row.run_id, invocation).await? == 0 {
            return Err(ChatError::Conflict);
        }
        super::activity(
            self.pg()?,
            &tx,
            row.thread_id,
            "invocation.suspend_requested",
            invocation,
            "",
        )
        .await?;
        tx.commit().await?;
        drop(tx_client);
        Ok(())
    }
    async fn control_commands(&self, claims: &Claims) -> Result<Vec<Command>> {
        if let Some(local) = self.local() {
            let invocation = local.invocation(claims.invocation_id).await?;
            if invocation.status != "running"
                || invocation.lease_expires_at <= chrono::Utc::now().timestamp_millis()
            {
                return Ok(vec![terminal(claims.invocation_id, Kind::Stop)]);
            }
            if local.run(claims.run_id).await?.status == "suspending" {
                return Ok(vec![terminal(claims.invocation_id, Kind::Suspend)]);
            }
        } else {
            let state = crate::chat::db::controls_state_opt(
                &self.pg()?.get().await?,
                claims.invocation_id,
                claims.sub,
                claims.thread_id,
                claims.run_id,
            )
            .await?;
            let Some(state) = state else {
                return Ok(vec![terminal(claims.invocation_id, Kind::Stop)]);
            };
            if state.status != "running" || !state.lease_live {
                return Ok(vec![terminal(claims.invocation_id, Kind::Stop)]);
            }
            if state.run_status == "suspending" {
                return Ok(vec![terminal(claims.invocation_id, Kind::Suspend)]);
            }
            // Gateway inputs are dispatched by the agent concurrency policy.
        }
        Ok(vec![])
    }
    async fn control_changes(&self, claims: &Claims) -> Result<Changes> {
        if let Some(local) = self.local() {
            let thread = claims.thread_id;
            let mut changes = local.changes();
            Ok(Box::pin(async_stream::try_stream! {
                loop {
                    match changes.recv().await {
                        Ok(changed) if changed == thread => yield (),
                        Ok(_) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => yield (),
                        Err(_) => break,
                    }
                }
            }))
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
        let commands = self.control_commands(&claims).await?;
        let command = commands.iter().find(|c| c.id == key.to_string());
        if let Some(local) = self.local() {
            return if local
                .acknowledge_control(claims.thread_id, claims.invocation_id, key)
                .await?
            {
                Ok(())
            } else {
                Err(ChatError::Denied)
            };
        }
        let mut tx_client = self.pg()?.get().await?;
        let tx = tx_client.transaction().await?;
        if command.is_none() {
            let exists = crate::chat::db::controls_receipt_one(&tx, claims.invocation_id, key)
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
            crate::chat::db::input_accept_execute(&tx, claims.invocation_id, key).await?;
        }
        if crate::chat::db::controls_ack_opt(&tx, claims.invocation_id, key)
            .await?
            .is_some()
        {
            super::activity(
                self.pg()?,
                &tx,
                claims.thread_id,
                "command.acknowledged",
                key,
                "",
            )
            .await?;
        }
        tx.commit().await?;
        drop(tx_client);
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
                let commands=chat.control_commands(&claims).await?;
                if !commands.iter().any(|c|matches!(c.kind.as_known(),Some(Kind::Stop|Kind::Suspend))) && !ready {ready=true;yield Command{kind:Kind::Ready.into(),..Default::default()};}
                seen.retain(|id|commands.iter().any(|c|&c.id==id));
                for command in commands {let terminal=matches!(command.kind.as_known(),Some(Kind::Stop|Kind::Suspend));if seen.insert(command.id.clone()){yield command;}
                    if terminal{return;}}
                // The open command stream is the host's liveness for this invocation: it
                // renews the lease that expiry would otherwise reclaim.
                let renew=tokio::time::sleep(Duration::from_secs(10));
                let update=tokio::select!{_= &mut deadline=>None,value=changed.next()=>value,_=renew=>Some(chat.renew_lease(claims.invocation_id).await)};
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
