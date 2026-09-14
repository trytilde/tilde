//! Registry calls from an agent beside a sidecar. The replica already verified
//! the live invocation; the gateway verifies the same token against the thread
//! lease, its projected state and the capability ceiling before running the
//! registry handler.
use super::{Deployments, id};
use crate::{chat::Scope, error::Error, proto::tilde::agent_event_ingress::v1 as wire};
use secrecy::ExposeSecret;
use serde::Deserialize;
use tower::ServiceExt;
use uuid::Uuid;
#[derive(Deserialize)]
struct RuntimeClaims {
    sub: Uuid,
    invocation_id: Uuid,
    run_id: Uuid,
    thread_id: Uuid,
    participant_id: Uuid,
    capabilities: crate::iam::capabilities::Capabilities,
    agent_generation: i64,
}
impl Deployments {
    pub async fn relay(
        &self,
        agent: Uuid,
        r: wire::RelayRequest,
    ) -> Result<wire::CallResult, Error> {
        let instance = id(&r.instance_id)?;
        let method = r
            .path
            .strip_prefix("tilde.runtime.v1.AgentService/")
            .ok_or(Error::Denied)?;
        if method.is_empty() || !method.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err(Error::Denied);
        }
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
        validation.set_issuer(&["tilde:invocation"]);
        validation.set_audience(&["tilde:agent-api"]);
        let key = self.signing_key(agent).await?;
        let claims = jsonwebtoken::decode::<RuntimeClaims>(
            &r.caller_token,
            &jsonwebtoken::DecodingKey::from_secret(key.expose_secret().as_bytes()),
            &validation,
        )
        .map_err(|_| Error::Invalid("Registry relay: invocation token signature rejected".into()))?
        .claims;
        let deployment = sqlx::query_file!("../../queries/deployment/get.sql", agent)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(Error::Denied)?;
        if claims.sub != agent
            || deployment.paused
            || deployment.generation != claims.agent_generation
        {
            return Err(Error::Invalid(
                "Registry relay: agent generation changed".into(),
            ));
        }
        let holder = self
            .holder(agent, claims.thread_id)
            .await?
            .ok_or_else(|| Error::Invalid("Registry relay: conversation has no holder".into()))?;
        if holder.instance != instance {
            return Err(Error::Invalid(
                "Registry relay: replica does not hold the conversation".into(),
            ));
        }
        // The holder verified liveness locally; projection may lag a young invocation
        // or even the thread's roster. Only an invocation the gateway already knows
        // to be over is refused, and the projected participant wins over the claim.
        let participant = sqlx::query_file!(
            "../../queries/deployment/thread_participant.sql",
            claims.thread_id,
            agent
        )
        .fetch_optional(&self.pool)
        .await?
        .map(|r| r.id)
        .unwrap_or(claims.participant_id);
        let projected = sqlx::query_file!(
            "../../queries/deployment/relay_invocation.sql",
            claims.invocation_id,
            agent,
            claims.thread_id,
            claims.run_id
        )
        .fetch_optional(&self.pool)
        .await?;
        if projected
            .as_ref()
            .is_some_and(|row| !matches!(row.status.as_str(), "pending" | "running"))
        {
            return Err(Error::Denied);
        }
        let record = self.agents.get(agent).await?;
        let scope = Scope {
            capabilities: claims.capabilities.intersect(&record.capabilities.0),
            id: claims.invocation_id,
            run_id: claims.run_id,
            thread_id: claims.thread_id,
            agent_id: agent,
            participant_id: projected
                .and_then(|row| row.participant_id)
                .unwrap_or(participant),
        };
        let mut request = http::Request::builder()
            .method(http::Method::POST)
            .uri(format!("/{}", r.path));
        if let Ok(value) = http::HeaderValue::from_str(&r.content_type) {
            request = request.header(http::header::CONTENT_TYPE, value);
        }
        let mut request = request
            .body(axum::body::Body::from(r.body))
            .map_err(|_| Error::Denied)?;
        request
            .extensions_mut()
            .insert(crate::iam::Principal::Agent {
                id: agent,
                invocation_id: claims.invocation_id,
            });
        request.extensions_mut().insert(scope);
        let response = match crate::agent::rpc::runtime::router(self.agents.clone(), self.chat())
            .oneshot(request)
            .await
        {
            Ok(response) => response,
            Err(never) => match never {},
        };
        let status = response.status().as_u16() as i32;
        let content_type = response
            .headers()
            .get(http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        let body = axum::body::to_bytes(response.into_body(), 4 * 1024 * 1024)
            .await
            .map(|b| b.to_vec())
            .unwrap_or_default();
        Ok(wire::CallResult {
            status,
            content_type,
            body,
            ..Default::default()
        })
    }
}
