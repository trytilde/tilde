//! Raw OTLP is replicated through Corrosion only until the gateway accepts it.
//! Langfuse credentials and external export exist only at the gateway.
use super::{Deployments, corrosion::client::statement, runtime::Runtime, sidecar::Node};
use crate::{
    chat::{ChatError, Result},
    error::Error,
    iam::tokens::Claims,
};
use axum::{
    Router,
    extract::{Request, State},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures::StreamExt;
use opentelemetry_proto::tonic::{
    collector::trace::v1::{ExportTraceServiceRequest, ExportTraceServiceResponse},
    common::v1::{AnyValue, KeyValue, any_value::Value},
    trace::v1::ResourceSpans,
};
use prost::Message as _;
use rotel::{
    bounded_channel::bounded,
    receivers::{otlp::otlp_http::build_service, otlp_output::OTLPOutput},
    topology::{
        batch::BatchConfig,
        fanout::Fanout,
        generic_pipeline::{Inspect, Pipeline},
        payload::{Ack, ExporterError, Message, RequestContext},
        processors::Processors,
    },
};
use secrecy::ExposeSecret;
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
    time::Duration,
};
use tower::ServiceExt;
use uuid::Uuid;
#[derive(Deserialize)]
struct TraceRow {
    id: String,
    thread_id: String,
    invocation_id: String,
    payload: String,
}
#[derive(Deserialize)]
struct HealthRow {
    id: String,
    agent_id: String,
    instance_id: String,
    checked_at: i64,
    healthy: i64,
    latency_ms: i32,
}
pub(crate) fn normalize(
    body: &[u8],
    json: bool,
    claims: &Claims,
    parent: Option<&str>,
) -> Result<ExportTraceServiceRequest> {
    let mut request = if json {
        serde_json::from_slice::<ExportTraceServiceRequest>(body)
            .map_err(|_| ChatError::Invalid("Invalid OTLP JSON".into()))?
    } else {
        ExportTraceServiceRequest::decode(body)
            .map_err(|_| ChatError::Invalid("Invalid OTLP Protobuf".into()))?
    };
    let mut count = 0;
    for resource in &mut request.resource_spans {
        for scope in &mut resource.scope_spans {
            for span in &mut scope.spans {
                count += 1;
                if count > 4096
                    || span.trace_id.len() != 16
                    || span.trace_id.iter().all(|b| *b == 0)
                    || span.span_id.len() != 8
                    || span.span_id.iter().all(|b| *b == 0)
                    || !matches!(span.parent_span_id.len(), 0 | 8)
                    || span.end_time_unix_nano < span.start_time_unix_nano
                {
                    return Err(ChatError::Invalid("Invalid OTLP span".into()));
                }
                if let Some(parent) = parent {
                    let fields = parent.split('-').collect::<Vec<_>>();
                    if fields.len() != 4
                        || fields[1] != hex::encode(&span.trace_id)
                        || fields[2] == hex::encode(&span.span_id)
                    {
                        return Err(ChatError::Denied);
                    }
                }
                span.attributes.retain(|a| !a.key.starts_with("tilde."));
                for (key, id) in [
                    ("tilde.agent.id", claims.sub),
                    ("tilde.thread.id", claims.thread_id),
                    ("tilde.run.id", claims.run_id),
                    ("tilde.invocation.id", claims.invocation_id),
                ] {
                    span.attributes.push(KeyValue {
                        key: key.into(),
                        value: Some(AnyValue {
                            value: Some(Value::StringValue(id.to_string())),
                        }),
                        ..Default::default()
                    });
                }
            }
        }
    }
    Ok(request)
}
fn pieces(request: &ExportTraceServiceRequest, agent: Uuid) -> Vec<(Uuid, Vec<u8>)> {
    let mut out = vec![];
    for resource in &request.resource_spans {
        for scope in &resource.scope_spans {
            for span in &scope.spans {
                let mut key = span.trace_id.clone();
                key.extend(&span.span_id);
                let id = Uuid::new_v5(&agent, &key);
                let mut scope = scope.clone();
                scope.spans = vec![span.clone()];
                let mut resource = resource.clone();
                resource.scope_spans = vec![scope];
                out.push((
                    id,
                    ExportTraceServiceRequest {
                        resource_spans: vec![resource],
                    }
                    .encode_to_vec(),
                ));
            }
        }
    }
    out
}

pub async fn persist_local(runtime: &Runtime, request: &ExportTraceServiceRequest) -> Result<()> {
    if !runtime.configuration().await?.tracing_enabled {
        return Ok(());
    }
    let _lock = runtime.mutation.lock().await;
    #[derive(Deserialize)]
    struct Size {
        bytes: i64,
    }
    let pending = runtime
        .client
        .query::<Size>(
            "SELECT COALESCE(SUM(length(payload)),0) AS bytes FROM traces",
            vec![],
        )
        .await?
        .first()
        .map(|r| r.bytes)
        .unwrap_or(0);
    let pieces = pieces(request, runtime.agent_id);
    if pending
        + pieces
            .iter()
            .map(|(_, v)| v.len() as i64 * 4 / 3 + 4)
            .sum::<i64>()
        > 64 * 1024 * 1024
    {
        return Err(ChatError::Invalid("Sidecar trace relay is full".into()));
    }
    let mut writes = Vec::new();
    for (id, bytes) in pieces {
        let value = ExportTraceServiceRequest::decode(bytes.as_slice())
            .map_err(|_| ChatError::Transport)?;
        let attrs = &value.resource_spans[0].scope_spans[0].spans[0].attributes;
        let text = |key: &str| {
            attrs
                .iter()
                .find(|a| a.key == key)
                .and_then(|a| a.value.as_ref())
                .and_then(|a| match &a.value {
                    Some(Value::StringValue(s)) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_default()
        };
        writes.push(statement("INSERT INTO traces(id,thread_id,invocation_id,created_at,payload) VALUES(?,?,?,?,?) ON CONFLICT(id) DO NOTHING",vec![json!(id),json!(text("tilde.thread.id")),json!(text("tilde.invocation.id")),json!(chrono::Utc::now().timestamp_millis()),json!(STANDARD.encode(bytes))]));
    }
    if !writes.is_empty() {
        runtime.client.transaction(writes).await?;
    }
    Ok(())
}
fn message_request(
    message: &Message<ResourceSpans>,
) -> std::result::Result<ExportTraceServiceRequest, u16> {
    let Some(RequestContext::Http(h)) = &message.request_context else {
        return Err(400);
    };
    let id = |key: &str| {
        h.get(key)
            .ok_or(400u16)?
            .parse::<Uuid>()
            .map_err(|_| 400u16)
    };
    let claims = Claims {
        iss: String::new(),
        aud: String::new(),
        sub: id("x-tilde-trace-agent")?,
        thread_id: id("x-tilde-trace-thread")?,
        invocation_id: id("x-tilde-trace-invocation")?,
        run_id: id("x-tilde-trace-run")?,
        capabilities: Default::default(),
        iat: 0,
        exp: 0,
    };
    let parent = h
        .get("x-tilde-trace-parent")
        .filter(|s| !s.is_empty())
        .map(String::as_str);
    normalize(
        &ExportTraceServiceRequest {
            resource_spans: message.payload.clone(),
        }
        .encode_to_vec(),
        false,
        &claims,
        parent,
    )
    .map_err(|_| 400)
}
struct Quiet;
impl Inspect<ResourceSpans> for Quiet {
    fn inspect(&self, _: &[ResourceSpans]) {}
    fn inspect_with_prefix(&self, _: Option<String>, _: &[ResourceSpans]) {}
}
pub fn router(node: Node) -> Router {
    let (tx, rx) = bounded(32);
    let (out_tx, mut out_rx) = bounded::<Vec<Message<ResourceSpans>>>(8);
    let mut pipeline = Pipeline::new(
        "traces",
        rx,
        Fanout::new("traces", vec![("sidecar", out_tx)]),
        None,
        BatchConfig {
            max_size: 512,
            timeout: Duration::from_millis(200),
            disabled: false,
        },
        Processors::empty(),
        vec![],
    );
    tokio::spawn(async move {
        let _ = pipeline
            .start(Quiet, tokio_util::sync::CancellationToken::new())
            .await;
    });
    let runtime = node.runtime.clone();
    tokio::spawn(async move {
        while let Some(batch) = out_rx.next().await {
            for message in batch {
                let request = match message_request(&message) {
                    Ok(value) => value,
                    Err(code) => {
                        if let Some(meta) = &message.metadata {
                            meta.reject(code).await;
                        }
                        continue;
                    }
                };
                match persist_local(&runtime, &request).await {
                    Ok(()) => {
                        if let Some(meta) = &message.metadata {
                            let _ = meta.ack().await;
                        }
                    }
                    Err(_) => {
                        if let Some(meta) = &message.metadata {
                            let _ = meta
                                .nack(ExporterError::ExportFailed {
                                    error_code: 503,
                                    error_message: "Trace relay unavailable".into(),
                                })
                                .await;
                        }
                    }
                }
            }
        }
    });
    let output = OTLPOutput::new(tx)
        .with_ack()
        .with_validator(|m| message_request(m).map(|_| ()));
    let service = build_service(
        Some(output),
        None,
        None,
        "/v1/traces".into(),
        "/v1/metrics".into(),
        "/v1/logs".into(),
        true,
        vec![
            "x-tilde-trace-agent".into(),
            "x-tilde-trace-thread".into(),
            "x-tilde-trace-invocation".into(),
            "x-tilde-trace-run".into(),
            "x-tilde-trace-parent".into(),
        ],
    );
    let service = tower::service_fn(move |request: Request| {
        let service = service.clone();
        async move {
            Ok::<_, std::convert::Infallible>(match service.oneshot(request).await {
                Ok(response) => response.map(axum::body::Body::new).into_response(),
                Err(_) => http::StatusCode::SERVICE_UNAVAILABLE.into_response(),
            })
        }
    });
    Router::new()
        .route_service("/v1/traces", service)
        .merge(super::logs::router(node.clone()))
        .layer(middleware::from_fn_with_state(node, authorize))
}
async fn authorize(State(node): State<Node>, mut request: Request, next: Next) -> Response {
    let token = request
        .headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));
    let Some(token) = token else {
        return http::StatusCode::UNAUTHORIZED.into_response();
    };
    let claims = match node.runtime.verified_claims(token, true).await {
        Ok(claims) => claims,
        Err(ChatError::NotFound) => {
            return node.gateway.proxy(node.runtime.agent_id, request).await;
        }
        Err(_) => return http::StatusCode::UNAUTHORIZED.into_response(),
    };
    let parent = node
        .runtime
        .invocation(claims.invocation_id)
        .await
        .map(|v| v.traceparent)
        .unwrap_or_default();
    for (key, value) in [
        ("x-tilde-trace-agent", claims.sub.to_string()),
        ("x-tilde-trace-thread", claims.thread_id.to_string()),
        ("x-tilde-trace-invocation", claims.invocation_id.to_string()),
        ("x-tilde-trace-run", claims.run_id.to_string()),
        ("x-tilde-trace-parent", parent),
    ] {
        let Ok(value) = value.parse() else {
            return http::StatusCode::BAD_REQUEST.into_response();
        };
        request.headers_mut().insert(key, value);
    }
    request.headers_mut().remove(http::header::AUTHORIZATION);
    match tokio::time::timeout(Duration::from_secs(15), next.run(request)).await {
        Ok(response) => response,
        Err(_) => http::StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}
impl Deployments {
    async fn accept_traces(
        &self,
        mut request: ExportTraceServiceRequest,
    ) -> std::result::Result<(), Error> {
        let Some(queue) = &self.telemetry else {
            return Ok(());
        };
        crate::telemetry::mapping::normalize(&mut request.resource_spans);
        queue.accept(&request.encode_to_vec()).await
    }
    pub async fn replicate_telemetry(
        &self,
        source: &Runtime,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> std::result::Result<(), Error> {
        let mut traces = source
            .client
            .subscribe::<TraceRow>("SELECT * FROM traces", vec![])
            .await?;
        let mut logs = source
            .client
            .subscribe::<super::logs::Row>("SELECT * FROM logs", vec![])
            .await?;
        let mut health = source
            .client
            .subscribe::<HealthRow>("SELECT * FROM health", vec![])
            .await?;
        loop {
            tokio::select! {_=shutdown.changed()=>return Ok(()),
                item=traces.next()=>match item{Some(Ok(row))if !row.deleted=>{
                    let row=row.value;let bytes=STANDARD.decode(&row.payload).map_err(|_|Error::Invalid("Invalid replicated trace".into()))?;
                    let mut request=ExportTraceServiceRequest::decode(bytes.as_slice()).map_err(|_|Error::Invalid("Invalid replicated OTLP".into()))?;
                    // The authenticated replication group, not payload attributes, owns the agent scope.
                    for resource in &mut request.resource_spans {for scope in &mut resource.scope_spans {for span in &mut scope.spans {
                        span.attributes.retain(|a|!matches!(a.key.as_str(),"tilde.agent.id"|"tilde.thread.id"|"tilde.invocation.id"));
                        for (key,value) in [("tilde.agent.id",source.agent_id.to_string()),("tilde.thread.id",row.thread_id.clone()),("tilde.invocation.id",row.invocation_id.clone())] {if !value.is_empty(){span.attributes.push(KeyValue{key:key.into(),value:Some(AnyValue{value:Some(Value::StringValue(value))}),..Default::default()});}}
                    }}}
                    self.accept_traces(request).await?;
                    source.client.transaction(vec![statement("DELETE FROM traces WHERE id=?",vec![json!(row.id)])]).await?;
                },Some(Ok(_))=>{},_=>return Err(Error::Invalid("Trace subscription closed".into()))},
                item=logs.next()=>match item{Some(Ok(row)) if !row.deleted=>self.project_logs(source,row.value).await?,Some(Ok(_))=>{},_=>return Err(Error::Invalid("Log subscription closed".into()))},
                item=health.next()=>match item{Some(Ok(row))if !row.deleted=>{let row=row.value;if super::id(&row.agent_id)?!=source.agent_id{return Err(Error::Denied);}
                    sqlx::query_file!("../../queries/deployment/telemetry/health.sql",super::id(&row.id)?,source.agent_id,row.instance_id,chrono::DateTime::from_timestamp_millis(row.checked_at).ok_or(Error::Denied)?,row.healthy!=0,row.latency_ms).execute(&self.pool).await?;
                },Some(Ok(_))=>{},_=>return Err(Error::Invalid("Health subscription closed".into()))}
            }
        }
    }
    pub async fn trace_proxy(
        &self,
        agent: Uuid,
        request: Request,
    ) -> std::result::Result<Response, Error> {
        let is_logs = request.uri().path().ends_with("/v1/logs");
        let settings = sqlx::query_file!("../../queries/deployment/get.sql", agent)
            .fetch_one(&self.pool)
            .await?;
        let secret = self.open_secrets(
            agent,
            settings.encrypted_secrets.as_deref().ok_or(Error::Denied)?,
        )?;
        let token = request
            .headers()
            .get(http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .ok_or(Error::Denied)?;
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
        validation.leeway = 300;
        validation.set_issuer(&["tilde:invocation"]);
        validation.set_audience(&["tilde:agent-api"]);
        let claims = jsonwebtoken::decode::<super::proxy::RuntimeClaims>(
            token,
            &jsonwebtoken::DecodingKey::from_secret(secret.signing_key.expose_secret().as_bytes()),
            &validation,
        )
        .map_err(|_| Error::Denied)?
        .claims;
        if claims.sub != agent {
            return Err(Error::Denied);
        }
        if !sqlx::query_file!(
            "../../queries/iam/invocation_trace_live.sql",
            claims.invocation_id,
            agent,
            claims.thread_id,
            claims.run_id,
            claims.exp as f64
        )
        .fetch_one(&self.pool)
        .await?
        .live
        {
            return Err(Error::Denied);
        }
        let json = request
            .headers()
            .get(http::header::CONTENT_TYPE)
            .is_some_and(|h| h.as_bytes().starts_with(b"application/json"));
        let body = axum::body::to_bytes(request.into_body(), 8 * 1024 * 1024)
            .await
            .map_err(|_| Error::Invalid("Trace upload too large".into()))?;
        let claims = Claims {
            iss: claims.iss,
            aud: claims.aud,
            sub: agent,
            thread_id: claims.thread_id,
            invocation_id: claims.invocation_id,
            run_id: claims.run_id,
            capabilities: claims.capabilities,
            iat: claims.iat,
            exp: claims.exp,
        };
        if is_logs {
            let value = super::logs::normalized(&body, json, &claims)?;
            if let Some(delivery) = &self.logs {
                delivery.accept(agent.to_string(), &value).await?;
            }
            return Ok(if json {
                axum::Json(serde_json::json!({})).into_response()
            } else {
                (
                    [("content-type", "application/x-protobuf")],
                    Vec::<u8>::new(),
                )
                    .into_response()
            });
        }
        let value = normalize(&body, json, &claims, None)?;
        self.accept_traces(value).await?;
        Ok(if json {
            axum::Json(serde_json::json!({})).into_response()
        } else {
            (
                [("content-type", "application/x-protobuf")],
                ExportTraceServiceResponse::default().encode_to_vec(),
            )
                .into_response()
        })
    }
}
impl Deployments {
    /// Reap archived health and completed retirement records. Trace payloads are
    /// removed separately, after durable gateway queue acceptance.
    pub async fn reap_telemetry(&self) -> std::result::Result<(), Error> {
        for agent in sqlx::query_file!("../../queries/deployment/project/candidates.sql")
            .fetch_all(&self.pool)
            .await?
        {
            let Some(peer) = self.peer(agent.id).await? else {
                continue;
            };
            let cutoff = chrono::Utc::now()
                .timestamp_millis()
                .saturating_sub(i64::from(self.get(agent.id).await?.retention_days) * 86_400_000);
            let health = peer
                .client
                .query::<super::runtime::IdRow>(
                    "SELECT id FROM health WHERE checked_at<? LIMIT 1000",
                    vec![json!(cutoff)],
                )
                .await?;
            let health_ids = health
                .iter()
                .map(|r| super::id(&r.id))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            #[derive(Deserialize)]
            struct RetirementProof {
                epoch: String,
                thread_id: String,
            }
            let retired = peer
                .client
                .query::<RetirementProof>(
                    "SELECT epoch,thread_id FROM retirements WHERE phase='committed'",
                    vec![],
                )
                .await?;
            let mut deletes = vec![];
            for row in sqlx::query_file!(
                "../../queries/deployment/telemetry/archived_health.sql",
                agent.id,
                &health_ids
            )
            .fetch_all(&self.pool)
            .await?
            {
                deletes.push(statement(
                    "DELETE FROM health WHERE id=?",
                    vec![json!(row.id)],
                ));
            }
            for proof in retired {
                let thread = super::id(&proof.thread_id)?;
                if sqlx::query_file!(
                    "../../queries/deployment/project/location.sql",
                    thread,
                    agent.id
                )
                .fetch_optional(&self.pool)
                .await?
                .is_some_and(|r| r.storage == "postgres")
                {
                    deletes.push(statement(
                        "DELETE FROM retirements WHERE epoch=?",
                        vec![json!(proof.epoch)],
                    ));
                }
            }
            if !deletes.is_empty() {
                peer.client.transaction(deletes).await?;
            }
        }
        Ok(())
    }
}

pub type PlatformRoutes = Arc<RwLock<BTreeMap<Uuid, Arc<Runtime>>>>;
/// One process-wide provider routes internal spans by their server-assigned agent ID.
pub fn platform(
    routes: PlatformRoutes,
) -> (
    opentelemetry_sdk::trace::SdkTracerProvider,
    tokio::task::JoinHandle<()>,
) {
    let (tx, mut rx) = bounded::<Message<ResourceSpans>>(32);
    let provider = crate::telemetry::platform_provider(tx, true, "tilde-sidecar");
    let worker = tokio::spawn(async move {
        while let Some(message) = rx.next().await {
            for resource in message.payload {
                for scope in &resource.scope_spans {
                    for span in &scope.spans {
                        let agent = span
                            .attributes
                            .iter()
                            .find(|a| a.key == "tilde.agent.id")
                            .and_then(|a| a.value.as_ref())
                            .and_then(|a| match &a.value {
                                Some(Value::StringValue(v)) => v.parse::<Uuid>().ok(),
                                _ => None,
                            });
                        let runtime = agent.and_then(|id| routes.read().ok()?.get(&id).cloned());
                        if let Some(runtime) = runtime {
                            let resource = ResourceSpans {
                                resource: resource.resource.clone(),
                                schema_url: resource.schema_url.clone(),
                                scope_spans: vec![
                                    opentelemetry_proto::tonic::trace::v1::ScopeSpans {
                                        scope: scope.scope.clone(),
                                        schema_url: scope.schema_url.clone(),
                                        spans: vec![span.clone()],
                                    },
                                ],
                            };
                            if persist_local(
                                &runtime,
                                &ExportTraceServiceRequest {
                                    resource_spans: vec![resource],
                                },
                            )
                            .await
                            .is_err()
                            {
                                tracing::warn!(agent_id=%runtime.agent_id,"Sidecar platform telemetry could not be retained");
                            }
                        }
                    }
                }
            }
        }
    });
    (provider, worker)
}
/// Node routing already determines the agent; never infer it from incoming attributes.
pub async fn capture(State(node): State<Node>, mut request: Request, next: Next) -> Response {
    request
        .extensions_mut()
        .insert(crate::telemetry::context::AgentOwner(node.runtime.agent_id));
    crate::telemetry::context::request(request, next).await
}
