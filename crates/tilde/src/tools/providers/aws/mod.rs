//! AWS through the official AWS MCP Server. The tool list is a fixed snapshot of that server's
//! catalog (`catalog.json`), so tools are offered without discovery; each call opens an MCP
//! session whose every request is SigV4-signed for `aws-mcp` in us-east-1 with the connection's
//! IAM credentials. The connection's region reaches the server as `_meta.AWS_REGION`, the
//! default for operations that name no region.
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

pub fn definition() -> model::Provider {
    model::Provider {
        account_name_label: Some("AWS account or role".into()),
        icon_url: Some("/provider-icons/aws.svg".into()),
        instructions: Some(
            "AWS cloud operations, regional availability, documentation, skills, file transfer, and sandboxed multi-step API execution through the official AWS MCP Server. Requests are authenticated with encrypted AWS IAM credentials and SigV4."
                .into(),
        ),
        id: "aws".into(),
        name: "AWS".into(),
        kind: model::ProviderKind::BuiltIn,
        categories: vec![CATEGORY_CLOUD_INFRASTRUCTURE.into()],
        connection_types: vec![model::ConnectionType {
            mcp: None,
            id: "iam".into(),
            name: "AWS IAM credentials".into(),
            credential_source: model::CredentialSource::Static {
                schema: json!({"type":"object","properties":{
                    "access_key_id":{"type":"string","title":"Access key ID","minLength":1,"description":"Access key of the IAM principal or temporary session."},
                    "secret_access_key":{"type":"string","title":"Secret access key","minLength":1,"writeOnly":true},
                    "session_token":{"type":"string","title":"Session token","writeOnly":true,"description":"Required when the access key belongs to a temporary session."},
                    "region":{"type":"string","title":"Default region","default":SIGNING_REGION,"pattern":"^[a-z]{2}(-[a-z]+)+-\\d$","description":"Used by tools when an operation names no region."}
                },"required":["access_key_id","secret_access_key"],"additionalProperties":false}),
            },
            capabilities: vec![model::Capability::Tool],
        }],
    }
}

pub struct Aws;

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

impl ToolProvider for Aws {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        catalog()
            .iter()
            .map(|tool| {
                let name = local_name(tool["name"].as_str().unwrap_or_default());
                let hints = &tool["annotations"];
                types::ToolDefinition {
                    summary: summary(&name).into(),
                    name,
                    provider_id: "aws".into(),
                    description: tool["description"].as_str().unwrap_or_default().into(),
                    input_schema_json: tool["inputSchema"].to_string(),
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
            .collect()
    }
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        _call_id: uuid::Uuid,
        name: &'a str,
        input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            let upstream = catalog()
                .iter()
                .filter_map(|tool| tool["name"].as_str())
                .find(|upstream| local_name(upstream) == name)
                .ok_or_else(|| ConnectError::not_found("Unsupported AWS tool"))?;
            let values = &access.values;
            // Credentials zeroize when the signer drops at the end of the call.
            let signer = client_aws_sigv4::Signer::new(
                client_aws_sigv4::Credentials {
                    access_key_id: access.secret("access_key_id")?.to_owned(),
                    secret_access_key: access.secret("secret_access_key")?.to_owned(),
                    session_token: model::optional(values, "session_token").map(str::to_owned),
                },
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
