//! Log batches use the existing Corrosion group; no independent replication transport.
use super::{Deployments, corrosion::client::statement, runtime::Runtime, sidecar::Node};
use crate::{
    chat::{ChatError, Result},
    error::Error,
    iam::tokens::Claims,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use opentelemetry_proto::tonic::{
    collector::logs::v1::ExportLogsServiceRequest, common::v1::any_value::Value,
};
use prost::Message as _;
use rotel::topology::payload::{Ack, ExporterError, Message, RequestContext};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;
#[derive(Deserialize)]
pub(super) struct Row {
    pub id: String,
    pub created_at: i64,
    pub payload: String,
}

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
pub async fn persist(runtime: &Runtime, request: ExportLogsServiceRequest) -> Result<()> {
    let _lock = runtime.mutation.lock().await;
    if !runtime.configuration().await?.logs_enabled {
        return Ok(());
    }
    #[derive(Deserialize)]
    struct Size {
        bytes: i64,
    }
    let cutoff = chrono::Utc::now().timestamp_millis() - 86_400_000;
    let expired = runtime
        .client
        .transaction(vec![statement(
            "DELETE FROM logs WHERE created_at<?",
            vec![json!(cutoff)],
        )])
        .await?;
    if expired.iter().sum::<u64>() > 0 {
        tracing::warn!(agent_id=%runtime.agent_id, batches=expired.iter().sum::<u64>(), "Expired undelivered sidecar log batches after 24 hours");
    }
    let bytes = request.encode_to_vec();
    let id = hex::encode(Sha256::digest(&bytes));
    let size = runtime
        .client
        .query::<Size>(
            "SELECT COALESCE(SUM(length(payload)),0) AS bytes FROM logs WHERE id<>?",
            vec![json!(&id)],
        )
        .await?
        .first()
        .map(|s| s.bytes)
        .unwrap_or_default();
    if bytes.len() > 8 * 1024 * 1024 || size + (bytes.len() as i64 * 4 / 3 + 4) > 64 * 1024 * 1024 {
        return Err(ChatError::Invalid("Sidecar logs queue full".into()));
    }
    runtime
        .client
        .transaction(vec![statement(
            "INSERT INTO logs(id,created_at,payload) VALUES(?,?,?) ON CONFLICT(id) DO NOTHING",
            vec![
                json!(id),
                json!(chrono::Utc::now().timestamp_millis()),
                json!(STANDARD.encode(bytes)),
            ],
        )])
        .await?;
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
                    Ok(_) => persist(
                        &runtime,
                        ExportLogsServiceRequest {
                            resource_logs: message.payload,
                        },
                    )
                    .await
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
impl Deployments {
    pub(super) async fn project_logs(
        &self,
        source: &Runtime,
        row: Row,
    ) -> std::result::Result<(), Error> {
        if row.created_at < chrono::Utc::now().timestamp_millis() - 86_400_000 {
            tracing::warn!(agent_id=%source.agent_id,"Expired sidecar log batch after 24 hours");
        } else if let Some(delivery) = &self.logs
            && delivery.enabled()
        {
            let bytes = STANDARD
                .decode(&row.payload)
                .map_err(|_| Error::Invalid("Invalid replicated log payload".into()))?;
            let request = ExportLogsServiceRequest::decode(bytes.as_slice())
                .map_err(|_| Error::Invalid("Invalid replicated OTLP logs".into()))?;
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
            if let Some((thread_id, run_id, invocation_id)) = identity {
                let claims = Claims {
                    iss: String::new(),
                    aud: String::new(),
                    sub: source.agent_id,
                    thread_id,
                    run_id,
                    invocation_id,
                    capabilities: Default::default(),
                    iat: 0,
                    exp: 0,
                };
                let request = normalized(&bytes, false, &claims)?;
                delivery
                    .accept(source.agent_id.to_string(), &request)
                    .await?;
            }
        }
        source
            .client
            .transaction(vec![statement(
                "DELETE FROM logs WHERE id=?",
                vec![json!(row.id)],
            )])
            .await?;
        Ok(())
    }
}
