//! Archived command execution never populates Corrosion conversation tables.
use super::sidecar::Node;
use crate::{
    chat::{ChatError, Result},
    proto::tilde::{agent_event_ingress::v1 as control, types::v1 as types},
};
use futures::StreamExt;
use secrecy::ExposeSecret;
use std::time::Duration;
impl Node {
    pub async fn gateway_commands(self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut tasks = tokio::task::JoinSet::new();
        let mut active = std::collections::BTreeSet::new();
        loop {
            if let Ok(mut commands) = self.gateway.watch_commands(self.runtime.instance_id).await {
                loop {
                    tokio::select! {
                        _=shutdown.changed()=>{tasks.abort_all();return;},
                        result=tasks.join_next(),if !tasks.is_empty()=>{if let Some(Ok(id))=result{active.remove(&id);}},
                        command=commands.next()=>match command{
                            Some(Ok(command))=>{
                                let key=(command.id.clone(),command.generation);
                                if command.agent_id==self.runtime.agent_id.to_string()&&command.owner_instance_id==self.runtime.instance_id.to_string()&&active.insert(key.clone()){
                                    let node=self.clone();tasks.spawn(async move{if node.execute_gateway_command(command).await.is_err(){tracing::warn!("Archived agent command failed");}key});
                                }
                            },_=>break,
                        }
                    }
                }
            }
            tokio::select! {_=shutdown.changed()=>{tasks.abort_all();return;},_=tokio::time::sleep(Duration::from_secs(1))=>{}}
        }
    }
    async fn acknowledge_gateway(&self, c: &types::AgentCommand) -> Result<()> {
        let _: control::AcknowledgeCommandResponse = self
            .gateway
            .rpc(
                "AcknowledgeCommand",
                &control::AcknowledgeCommandRequest {
                    command_id: c.id.clone(),
                    generation: c.generation,
                    instance_id: self.runtime.instance_id.to_string(),
                    ..Default::default()
                },
            )
            .await?;
        Ok(())
    }
    async fn execute_gateway_command(&self, command: types::AgentCommand) -> Result<()> {
        let mut pending = vec![];
        let result = async {
            match &command.action {
                Some(types::agent_command::Action::Invoke(_)) => {
                    let response: control::GetInvocationResponse = self
                        .gateway
                        .rpc(
                            "GetInvocation",
                            &control::GetInvocationRequest {
                                command_id: command.id.clone(),
                                generation: command.generation,
                                instance_id: self.runtime.instance_id.to_string(),
                                ..Default::default()
                            },
                        )
                        .await?;
                    let mut request = response
                        .invocation
                        .into_option()
                        .ok_or(ChatError::Transport)?;
                    request.callback_url = self.runtime.callback_url.clone();
                    let mut stream = crate::chat::runtime::client(
                        &self.runtime.local_endpoint,
                        self.runtime.host_key.expose_secret(),
                        "Invoke",
                        &request,
                    )?
                    .invoke(request)
                    .await
                    .map_err(|_| ChatError::Transport)?;
                    while let Some(message) =
                        stream.message().await.map_err(|_| ChatError::Transport)?
                    {
                        let message = message.view();
                        if message.accepted_command_id == command.id {
                            self.acknowledge_gateway(&command).await?;
                        }
                        if !message.reasoning_delta.is_empty() {
                            let _: control::ReportActivityResponse = self
                                .gateway
                                .rpc(
                                    "ReportActivity",
                                    &control::ReportActivityRequest {
                                        command_id: command.id.clone(),
                                        generation: command.generation,
                                        instance_id: self.runtime.instance_id.to_string(),
                                        reasoning_delta: message.reasoning_delta.into(),
                                        ..Default::default()
                                    },
                                )
                                .await?;
                        }
                        pending.extend(
                            message
                                .pending_input_ids
                                .iter()
                                .map(|value| (*value).to_owned()),
                        );
                    }
                }
                Some(_) => {
                    return Err(ChatError::Invalid(
                        "Only invocation dispatch belongs to the sidecar command stream".into(),
                    ));
                }
                None => return Err(ChatError::Invalid("Command has no action".into())),
            }
            Ok(())
        }
        .await;
        let _: control::CompleteCommandResponse = self
            .gateway
            .rpc(
                "CompleteCommand",
                &control::CompleteCommandRequest {
                    command_id: command.id,
                    generation: command.generation,
                    instance_id: self.runtime.instance_id.to_string(),
                    status: if result.is_ok() { "stopped" } else { "failed" }.into(),
                    pending_input_ids: pending,
                    ..Default::default()
                },
            )
            .await?;
        result
    }
}
