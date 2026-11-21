//! AWS through the official AWS MCP Server. The tool list is a fixed snapshot of that server's
//! catalog (`catalog.json`), so tools are offered without discovery; each call opens an MCP
//! session whose every request is SigV4-signed for `aws-mcp` in us-east-1 with the connection's
//! IAM credentials. The connection's region reaches the server as `_meta.AWS_REGION`, the
//! default for operations that name no region.
//!
//! Every tool takes an optional `account_id`: the call then runs with credentials for that
//! account (`sts`), and `aws_list_accounts` tells the agent which accounts it can reach. An agent
//! may also use several AWS connections (one per account), each its own tool source.
//!
//! The same connection emits CloudWatch alarm signals from an SNS topic's HTTPS subscription
//! (`signals::aws`); the IAM credentials play no part in that, the accepted topics do.
pub(crate) mod sts;
pub use sts::{AUDIENCE, session};

use super::ToolProvider;
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{categories::CATEGORY_CLOUD_INFRASTRUCTURE, model};
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use serde_json::{Value, json};

const ENDPOINT: &str = "https://aws-mcp.us-east-1.api.aws/mcp";
const SIGNING_SERVICE: &str = "aws-mcp";
const SIGNING_REGION: &str = "us-east-1";

/// Fields both connection types share: the default region, roles in other accounts, and the
/// SNS topics alarms arrive from.
fn shared_fields() -> serde_json::Map<String, Value> {
    json!({
        "region":{"type":"string","title":"Default region","default":SIGNING_REGION,"pattern":"^[a-z]{2}(-[a-z]+)+-\\d$","description":"Used by tools when an operation names no region, and for STS."},
        "assume_role_arns":{"type":"string","title":"Roles in other accounts","maxLength":4000,"description":"Optional. IAM role ARNs in other AWS accounts, separated by commas. A tool call naming one of those accounts assumes its role (session name tilde-<connection ID>); the role's trust policy must allow sts:AssumeRole by this connection's own identity."},
        "external_id":{"type":"string","title":"External ID","maxLength":1224,"writeOnly":true,"description":"Optional. Sent with every AssumeRole into the roles above, for trust policies that require sts:ExternalId."},
        "sns_topic_arns":{"type":"string","title":"Alarm SNS topic ARNs","maxLength":4000,"description":"Optional. To receive CloudWatch alarms as signals, subscribe this connection's webhook URL (HTTPS protocol) to the SNS topics your alarms notify, in any account or region, and list those topic ARNs here, separated by commas. Messages from other topics are refused; the subscription is confirmed automatically."}
    }).as_object().cloned().unwrap_or_default()
}
pub fn definition() -> model::Provider {
    let mut iam = json!({
        "access_key_id":{"type":"string","title":"Access key ID","minLength":1,"description":"Access key of the IAM principal or temporary session."},
        "secret_access_key":{"type":"string","title":"Secret access key","minLength":1,"writeOnly":true},
        "session_token":{"type":"string","title":"Session token","writeOnly":true,"description":"Required when the access key belongs to a temporary session."}
    }).as_object().cloned().unwrap_or_default();
    iam.extend(shared_fields());
    let mut oidc = json!({
        "role_arn":{"type":"string","title":"Role ARN","minLength":20,"pattern":"^arn:aws[a-z-]*:iam::[0-9]{12}:role/.+$","description":"The role Tilde assumes with its web identity token; see the trust policy above."}
    }).as_object().cloned().unwrap_or_default();
    oidc.extend(shared_fields());
    model::Provider {
        api_hosts: vec!["*.amazonaws.com".into()],
        account_name_label: Some("AWS account or role".into()),
        icon_url: Some("/provider-icons/aws.svg".into()),
        instructions: Some(
            "AWS cloud operations, regional availability, documentation, skills, file transfer, and sandboxed multi-step API execution through the official AWS MCP Server, in this account and in other accounts through roles it may assume. Connect with OpenID Connect (no keys: Tilde is the identity provider, as AWS recommends) or with IAM access keys. CloudWatch alarms reach routines as signals through SNS topics subscribed to this connection's webhook URL."
                .into(),
        ),
        id: "aws".into(),
        name: "AWS".into(),
        kind: model::ProviderKind::BuiltIn,
        categories: vec![CATEGORY_CLOUD_INFRASTRUCTURE.into()],
        connection_types: vec![
            model::ConnectionType {
                mcp: None,
                id: "oidc".into(),
                name: "AWS role with OpenID Connect".into(),
                credential_source: model::CredentialSource::Static {
                    schema: json!({"type":"object","properties":oidc,"required":["role_arn"],"additionalProperties":false}),
                },
                capabilities: vec![model::Capability::Tool, model::Capability::Signal],
            },
            model::ConnectionType {
                mcp: None,
                id: "iam".into(),
                name: "AWS IAM credentials".into(),
                credential_source: model::CredentialSource::Static {
                    schema: json!({"type":"object","properties":iam,"required":["access_key_id","secret_access_key"],"additionalProperties":false}),
                },
                capabilities: vec![model::Capability::Tool, model::Capability::Signal],
            },
        ],
    }
}

/// The connection type: `iam` (access keys) or `oidc` (web identity).
pub struct Aws(&'static str);
pub static IAM: Aws = Aws("iam");
pub static OIDC: Aws = Aws("oidc");
const LIST_ACCOUNTS: &str = "aws_list_accounts";

fn catalog() -> &'static [Value] {
    static CATALOG: std::sync::OnceLock<Vec<Value>> = std::sync::OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("catalog.json")).expect("embedded AWS MCP catalog")
    })
}
/// Upstream names are `aws___<name>`; Tilde's are `aws_<name>`.
fn local_name(upstream: &str) -> String {
    format!(
        "aws_{}",
        upstream.strip_prefix("aws___").unwrap_or(upstream)
    )
}
fn summary(name: &str) -> &'static str {
    match name {
        "aws_call_aws" => "Ran an AWS CLI command",
        "aws_get_presigned_url" => "Created an S3 transfer URL",
        "aws_get_tasks" => "Checked AWS tasks",
        "aws_run_script" => "Ran an AWS script",
        "aws_get_regional_availability" => "Checked AWS regional availability",
        "aws_list_regions" => "Listed AWS regions",
        "aws_read_documentation" => "Read AWS documentation",
        "aws_retrieve_skill" => "Retrieved an AWS skill",
        "aws_search_documentation" => "Searched AWS documentation",
        _ => "Called AWS",
    }
}

fn account_id() -> Value {
    json!({"type":"string","pattern":"^[0-9]{12}$","description":"AWS account to act in. Defaults to the connection's own account; another account must be one aws_list_accounts names."})
}
impl ToolProvider for Aws {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        let mut tools: Vec<_> = catalog()
            .iter()
            .map(|tool| {
                let name = local_name(tool["name"].as_str().unwrap_or_default());
                let hints = &tool["annotations"];
                let mut schema = tool["inputSchema"].clone();
                if let Some(properties) = schema["properties"].as_object_mut() {
                    properties.insert("account_id".into(), account_id());
                }
                types::ToolDefinition {
                    summary: summary(&name).into(),
                    name,
                    provider_id: "aws".into(),
                    description: tool["description"].as_str().unwrap_or_default().into(),
                    input_schema_json: schema.to_string(),
                    output_schema_json: tool
                        .get("outputSchema")
                        .unwrap_or(&json!({"type":"object"}))
                        .to_string(),
                    annotations: types::ToolAnnotations {
                        read_only: hints["readOnlyHint"].as_bool().unwrap_or(false),
                        destructive: hints["destructiveHint"].as_bool().unwrap_or(false),
                        idempotent: hints["idempotentHint"].as_bool().unwrap_or(false),
                        open_world: hints["openWorldHint"].as_bool().unwrap_or(false),
                        ..Default::default()
                    }
                    .into(),
                    ..Default::default()
                }
            })
            .collect();
        tools.push(super::rest::definition(
            "aws",
            LIST_ACCOUNTS,
            "Listed reachable AWS accounts",
            "List the AWS accounts this connection's tools can act in: its own (the default) and those of the roles it may assume. Pass one as account_id to any AWS tool.",
            json!({"type":"object","properties":{},"additionalProperties":false}),
            super::rest::Hints {
                read_only: true,
                destructive: false,
            },
        ));
        tools
    }
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        _call_id: uuid::Uuid,
        name: &'a str,
        input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            if name == LIST_ACCOUNTS {
                let roles = sts::roles(&access.values)?;
                return Ok(json!({
                    "default_account": sts::own_account(access, self.0).await?,
                    "accounts": roles.iter().map(|r| json!({"account_id": r.account, "role_arn": r.arn})).collect::<Vec<_>>(),
                }));
            }
            let upstream = catalog()
                .iter()
                .filter_map(|tool| tool["name"].as_str())
                .find(|upstream| local_name(upstream) == name)
                .ok_or_else(|| ConnectError::not_found("Unsupported AWS tool"))?;
            let values = &access.values;
            let mut input = input;
            let account = input
                .as_object_mut()
                .and_then(|i| i.remove("account_id"))
                .and_then(|a| a.as_str().map(str::to_owned));
            // Credentials zeroize when the signer drops at the end of the call.
            let signer = client_aws_sigv4::Signer::new(
                crate::connections::mints::mint(
                    access,
                    "aws",
                    self.0,
                    &crate::connections::mints::MintScope {
                        resources: account.into_iter().collect(),
                        permissions: vec![],
                    },
                    "tool",
                )
                .await?
                .aws()?,
                SIGNING_REGION,
                SIGNING_SERVICE,
            );
            let region = model::optional(values, "region").unwrap_or(SIGNING_REGION);
            let url = url::Url::parse(
                access
                    .endpoints
                    .0
                    .get("aws_mcp")
                    .map_or(ENDPOINT, String::as_str),
            )
            .map_err(|_| ConnectError::internal("Invalid provider endpoint"))?;
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| ConnectError::internal("Transport failed"))?;
            let result = crate::tools::mcp::call_signed(
                client,
                url,
                signer,
                json!({"name":upstream,"arguments":input,"_meta":{"AWS_REGION":region}}),
            )
            .await?;
            if let Some(structured) = result.get("structuredContent").filter(|s| !s.is_null()) {
                return Ok(structured.clone());
            }
            // Most AWS tools answer with one text part holding JSON.
            let content = result["content"].as_array().cloned().unwrap_or_default();
            if let [part] = content.as_slice()
                && let Some(parsed) = part["text"]
                    .as_str()
                    .and_then(|text| serde_json::from_str::<Value>(text).ok())
                    .filter(Value::is_object)
            {
                return Ok(parsed);
            }
            Ok(json!({"content": content}))
        })
    }
}
