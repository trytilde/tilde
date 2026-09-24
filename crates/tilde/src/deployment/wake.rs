//! Wakers: how the gateway starts an execution on a deployment that cannot hold a
//! connection. A wake carries the invocation; everything after it flows through the
//! run protocol the host dials.
use crate::{chat::ChatError, error::Error};
use std::time::SystemTime;

/// Invoke an AWS Lambda function asynchronously with the serialized wake as its event.
/// The reference is the function ARN; credentials come from the gateway's AWS
/// environment. The function's response is irrelevant: it reports through RunService.
pub async fn lambda(reference: &str, payload: &[u8]) -> Result<(), Error> {
    let region = reference
        .strip_prefix("arn:aws:lambda:")
        .and_then(|rest| rest.split(':').next())
        .filter(|region| !region.is_empty())
        .ok_or_else(|| Error::Invalid("Lambda deployments need a function ARN".into()))?;
    let config = client_aws_config::load(region)
        .await
        .map_err(|error| Error::Invalid(format!("AWS credentials for Lambda wake: {error}")))?;
    let signer = client_aws_sigv4::Signer::new(config.credentials, region, "lambda");
    let url = format!(
        "https://lambda.{region}.amazonaws.com/2015-03-31/functions/{}/invocations",
        client_aws_sigv4::aws_encode(reference.as_bytes())
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|_| ChatError::Transport)?;
    let mut request = client
        .post(url)
        .header("x-amz-invocation-type", "Event")
        .header("content-type", "application/json")
        .body(payload.to_vec())
        .build()
        .map_err(|_| ChatError::Transport)?;
    signer
        .sign_request_at(&mut request, payload, SystemTime::now())
        .map_err(|error| Error::Invalid(format!("Lambda wake signing failed: {error}")))?;
    let response = client
        .execute(request)
        .await
        .map_err(|_| ChatError::Transport)?;
    if !response.status().is_success() {
        tracing::warn!(status = %response.status(), "Lambda wake was not accepted");
        return Err(ChatError::Transport.into());
    }
    Ok(())
}
