//! Log batches use the same relay as traces; the gateway owns delivery queues.
use super::{runtime::Runtime, sidecar::Node};
use crate::{
    chat::{ChatError, Result},
    error::Error,
    iam::tokens::Claims,
    proto::tilde::agent_event_ingress::v1 as wire,
};
use opentelemetry_proto::tonic::{
    collector::logs::v1::ExportLogsServiceRequest, common::v1::any_value::Value,
};
use prost::Message as _;
use rotel::topology::payload::{Ack, ExporterError, Message, RequestContext};
use uuid::Uuid;
pub fn normalized(
    body: &[u8],
    json: bool,
    claims: &Claims,
) -> std::result::Result<ExportLogsServiceRequest, Error> {
    let request = if json {
        serde_json::from_slice::<ExportLogsServiceRequest>(body)
            .map_err(|_| Error::Invalid("Invalid OTLP logs JSON".into()))?
    } else {
        ExportLogsServiceRequest::decode(body)
            .map_err(|_| Error::Invalid("Invalid OTLP logs protobuf".into()))?
    };
    let headers = [
        ("x-tilde-log-scope", "invocation".into()),
        ("x-tilde-trace-agent", claims.sub.to_string()),
        ("x-tilde-trace-invocation", claims.invocation_id.to_string()),
        ("x-tilde-trace-thread", claims.thread_id.to_string()),
        ("x-tilde-trace-run", claims.run_id.to_string()),
        ("x-tilde-trace-parent", String::new()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect();
    let mut message = Message::new(
        None,
        request.resource_logs,
        Some(RequestContext::Http(headers)),
    );
    crate::logs::ingress::validate(&message)
        .map_err(|_| Error::Invalid("Invalid OTLP logs".into()))?;
    crate::logs::ingress::prepare(&mut message).map_err(|_| Error::Denied)?;
    Ok(ExportLogsServiceRequest {
        resource_logs: message.payload,
    })
}
pub fn relay(runtime: &Runtime, request: ExportLogsServiceRequest) -> Result<()> {
    if !runtime.configuration()?.logs_enabled {
        return Ok(());
    }
    let payload = request.encode_to_vec();
    if payload.len() > 8 * 1024 * 1024 {
        return Err(ChatError::Invalid("Log batch is too large".into()));
    }
    runtime.ensure_capacity()?;
    runtime.push(
        wire::Telemetry {
            kind: wire::TelemetryKind::Logs.into(),
            payload,
            ..Default::default()
        }
        .into(),
    );
    Ok(())
}
pub fn router(node: Node) -> axum::Router {
    let (sender, mut receiver, _task) =
        crate::logs::pipeline(tokio_util::sync::CancellationToken::new());
    let runtime = node.runtime.clone();
    tokio::spawn(async move {
        while let Some(batch) = receiver.next().await {
            for mut message in batch {
                let result = match crate::logs::ingress::prepare(&mut message) {
                    Ok(_) => relay(
                        &runtime,
                        ExportLogsServiceRequest {
                            resource_logs: message.payload,
                        },
                    )
                    .map_err(|_| 503),
                    Err(code) => Err(code),
                };
                if let Some(meta) = message.metadata {
                    match result {
                        Ok(()) => {
                            let _ = meta.ack().await;
                        }
                        Err(400) => meta.reject(400).await,
                        Err(_) => {
                            let _ = meta
                                .nack(ExporterError::ExportFailed {
                                    error_code: 503,
                                    error_message: "Sidecar log relay unavailable".into(),
                                })
                                .await;
                        }
                    }
                }
            }
        }
    });
    crate::logs::receiver(sender, true)
}
/// Verify a homogeneous log scope, then re-stamp authenticated ownership for delivery.
pub(crate) async fn accept(
    delivery: &crate::logs::Delivery,
    agent: Uuid,
    deployment: Uuid,
    request: ExportLogsServiceRequest,
) -> std::result::Result<(), Error> {
    // A publisher is authenticated as a deployment. Rebuild that ownership rather
    // than trusting payload IDs; mixed deployment/invocation batches are rejected.
    let mut scopes = request
        .resource_logs
        .iter()
        .flat_map(|r| &r.scope_logs)
        .flat_map(|s| &s.log_records)
        .map(|log| {
            log.attributes
                .iter()
                .find(|a| a.key == "tilde.log.scope")
                .and_then(|a| a.value.as_ref())
                .and_then(|v| match &v.value {
                    Some(Value::StringValue(v)) => Some(v.as_str()),
                    _ => None,
                })
                .unwrap_or("invocation")
        });
    let Some(scope) = scopes.next() else {
        return Ok(());
    };
    if scopes.any(|other| other != scope) {
        return Err(Error::Denied);
    }
    if scope == "deployment" {
        let headers = [
            ("x-tilde-trace-agent", agent.to_string()),
            ("x-tilde-deployment", deployment.to_string()),
            ("x-tilde-log-scope", "deployment".into()),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        let mut message = Message::new(
            None,
            request.resource_logs,
            Some(RequestContext::Http(headers)),
        );
        crate::logs::ingress::validate(&message)
            .map_err(|_| Error::Invalid("Invalid OTLP logs".into()))?;
        crate::logs::ingress::prepare(&mut message).map_err(|_| Error::Denied)?;
        return delivery
            .accept(
                agent.to_string(),
                &ExportLogsServiceRequest {
                    resource_logs: message.payload,
                },
            )
            .await;
    }
    if scope != "invocation" {
        return Err(Error::Denied);
    }
    let mut identity = None;
    for resource in &request.resource_logs {
        for scope in &resource.scope_logs {
            for log in &scope.log_records {
                let id = |key: &str| -> std::result::Result<Uuid, Error> {
                    log.attributes
                        .iter()
                        .find(|a| a.key == key)
                        .and_then(|a| a.value.as_ref())
                        .and_then(|v| match &v.value {
                            Some(Value::StringValue(v)) => Uuid::parse_str(v).ok(),
                            _ => None,
                        })
                        .ok_or(Error::Denied)
                };
                let ids = (
                    id("tilde.thread.id")?,
                    id("tilde.run.id")?,
                    id("tilde.invocation.id")?,
                );
                if identity.is_some_and(|previous| previous != ids) {
                    return Err(Error::Denied);
                }
                identity = Some(ids);
            }
        }
    }
    let Some((thread_id, run_id, invocation_id)) = identity else {
        return Ok(());
    };
    let claims = Claims {
        iss: String::new(),
        aud: String::new(),
        sub: agent,
        thread_id,
        run_id,
        invocation_id,
        participant_id: Uuid::nil(), // Telemetry attribution does not carry participant identity.
        capabilities: Default::default(),
        inference: vec![],
        iat: 0,
        exp: 0,
    };
    let request = normalized(&request.encode_to_vec(), false, &claims)?;
    delivery.accept(agent.to_string(), &request).await
}
