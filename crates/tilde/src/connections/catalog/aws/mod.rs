//! Setup guidance for an AWS role federated with Tilde's OIDC issuer (`aws`/`oidc`); the
//! provider definition and tools live in `tools::providers::aws`.
use super::runtime::Runtime;
use crate::connections::{model::ConnectionType, service::Connections};
use uuid::Uuid;

pub(crate) struct AwsOidc;

/// The trust policy a customer gives the role `connection` assumes: only Tilde's issuer, only
/// the STS audience, only this connection's subject.
pub fn trust_policy(issuer: &str, connection: Uuid) -> serde_json::Value {
    let provider = issuer
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    serde_json::json!({
        "Version": "2012-10-17",
        "Statement": [{
            "Effect": "Allow",
            "Principal": {"Federated": format!("arn:aws:iam::<ACCOUNT_ID>:oidc-provider/{provider}")},
            "Action": "sts:AssumeRoleWithWebIdentity",
            "Condition": {"StringEquals": {
                format!("{provider}:aud"): crate::tools::providers::aws::AUDIENCE,
                format!("{provider}:sub"): format!("connection:{connection}"),
            }},
        }],
    })
}

#[async_trait::async_trait]
impl Runtime for AwsOidc {
    fn instructions(&self, _typ: &ConnectionType) -> &'static [&'static str] {
        &[
            "Tilde signs in to AWS as an OpenID Connect identity provider, so no access keys are stored.",
        ]
    }
    fn connection_instructions(
        &self,
        service: &Connections,
        connection: Uuid,
        _typ: &ConnectionType,
    ) -> Vec<String> {
        let issuer = service.oidc().url();
        vec![
            format!(
                "In IAM > Identity providers, add an OpenID Connect provider with provider URL {issuer} and audience {}. AWS secures it with its trusted certificate authorities; if the console asks for a thumbprint, use its Get thumbprint button.",
                crate::tools::providers::aws::AUDIENCE
            ),
            format!(
                "Create (or edit) the role for this connection with this trust policy, replacing <ACCOUNT_ID> with your account ID. It only lets this connection (subject connection:{connection}) assume the role: {}",
                trust_policy(issuer, connection)
            ),
            "Attach the permissions the agent needs to the role and enter its ARN below. For other accounts, list roles whose trust policy allows sts:AssumeRole by this role.".into(),
        ]
    }
}
