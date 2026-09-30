//! Metric batches use the same relay as traces and logs; the gateway owns delivery queues.
//! The sidecar authorizes the upload, stamps ownership on every data point from the verified
//! deployment or invocation, and ships the batch as a Metrics frame in its publish stream.
use super::{runtime::Runtime, sidecar::Node};
use crate::{
    chat::{ChatError, Result},
    proto::tilde::agent_event_ingress::v1 as wire,
};
use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
use prost::Message as _;
use rotel::topology::payload::{Ack, ExporterError};
pub fn relay(runtime: &Runtime, request: ExportMetricsServiceRequest) -> Result<()> {
    runtime.configuration()?;
    let payload = request.encode_to_vec();
    if payload.len() > 8 * 1024 * 1024 {
        return Err(ChatError::Invalid("Metric batch is too large".into()));
    }
    runtime.ensure_capacity()?;
    runtime.push(
        wire::Telemetry {
            kind: wire::TelemetryKind::Metrics.into(),
            payload,
            ..Default::default()
        }
        .into(),
    );
    Ok(())
}
pub fn router(node: Node) -> axum::Router {
    let (sender, mut receiver, _task) =
        crate::telemetry::metrics::pipeline(tokio_util::sync::CancellationToken::new());
    let runtime = node.runtime.clone();
    tokio::spawn(async move {
        while let Some(batch) = receiver.next().await {
            for mut message in batch {
                let result = match crate::telemetry::metrics::ingress::prepare(&mut message) {
                    Ok(_) => relay(
                        &runtime,
                        ExportMetricsServiceRequest {
                            resource_metrics: message.payload,
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
                                    error_message: "Sidecar metric relay unavailable".into(),
                                })
                                .await;
                        }
                    }
                }
            }
        }
    });
    crate::telemetry::metrics::receiver(sender)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deployment::{gateway, runtime::RuntimeOptions};
    use opentelemetry_proto::tonic::{
        common::v1::{AnyValue, KeyValue, any_value::Value},
        metrics::v1::{
            Gauge, Metric, NumberDataPoint, ResourceMetrics, ScopeMetrics, metric::Data,
        },
    };
    use secrecy::SecretString;
    use std::{sync::Arc, time::Duration};
    use uuid::Uuid;

    fn payload() -> ExportMetricsServiceRequest {
        let forged = KeyValue {
            key: "tilde.agent.id".into(),
            value: Some(AnyValue {
                value: Some(Value::StringValue(Uuid::new_v4().to_string())),
            }),
            ..Default::default()
        };
        ExportMetricsServiceRequest {
            resource_metrics: vec![ResourceMetrics {
                scope_metrics: vec![ScopeMetrics {
                    metrics: vec![Metric {
                        name: "agent.queue.depth".into(),
                        data: Some(Data::Gauge(Gauge {
                            data_points: vec![NumberDataPoint {
                                time_unix_nano: 1_790_000_000_000_000_000,
                                attributes: vec![forged],
                                value: Some(
                                    opentelemetry_proto::tonic::metrics::v1::number_data_point::Value::AsInt(3),
                                ),
                                ..Default::default()
                            }],
                        })),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
        }
    }
    fn text(attributes: &[KeyValue], key: &str) -> Option<String> {
        attributes
            .iter()
            .find(|a| a.key == key)
            .and_then(|a| match &a.value {
                Some(AnyValue {
                    value: Some(Value::StringValue(v)),
                }) => Some(v.clone()),
                _ => None,
            })
    }

    /// A deployment-token upload to the sidecar's `/v1/metrics` becomes a Metrics frame owned
    /// by the verified deployment, exactly as logs do; disabled metrics are accepted and dropped.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn sidecar_metrics_are_authorized_stamped_and_relayed() {
        let token = "deployment-token";
        let runtime = Arc::new(Runtime::new(
            gateway::Client::new("http://127.0.0.1:1", SecretString::from(token)).unwrap(),
            RuntimeOptions {
                agent_id: Uuid::new_v4(),
                instance_id: Uuid::new_v4(),
                signing_key: SecretString::from("test-signing-key"),
                callback_url: "http://127.0.0.1:1".into(),
                idle_after: Duration::from_secs(60),
                cache_bytes: 1024 * 1024,
            },
        ));
        let deployment = Uuid::new_v4();
        let node = Node::for_tests(runtime.clone(), &deployment.to_string(), token);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/v1/metrics", listener.local_addr().unwrap());
        let router = super::super::telemetry::router(node);
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let client = reqwest::Client::new();
        let post = |token: &str| {
            client
                .post(&url)
                .bearer_auth(token)
                .header("content-type", "application/x-protobuf")
                .body(payload().encode_to_vec())
                .send()
        };
        let frames = || runtime.state.outbox.lock().unwrap().len();

        runtime.configure(wire::GetConfigurationResponse::default());
        assert_eq!(post("wrong").await.unwrap().status(), 401);
        assert_eq!(frames(), 0);
        assert_eq!(post(token).await.unwrap().status(), 200);
        let frame = runtime.state.outbox.lock().unwrap().pop_front().unwrap();
        let Some(wire::upstream::Frame::Telemetry(frame)) = frame.frame else {
            panic!("missing telemetry frame")
        };
        assert_eq!(frame.kind.as_known(), Some(wire::TelemetryKind::Metrics));
        let relayed = ExportMetricsServiceRequest::decode(frame.payload.as_slice()).unwrap();
        let Some(Data::Gauge(gauge)) =
            &relayed.resource_metrics[0].scope_metrics[0].metrics[0].data
        else {
            panic!("gauge survives the relay")
        };
        let attributes = &gauge.data_points[0].attributes;
        assert_eq!(
            text(attributes, "tilde.agent.id"),
            Some(runtime.agent_id.to_string()),
            "ownership comes from the node, not the payload"
        );
        assert_eq!(
            text(attributes, "tilde.deployment.id"),
            Some(deployment.to_string())
        );
        server.abort();
    }
}
