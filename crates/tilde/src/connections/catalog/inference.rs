//! Model providers are static-credential connections with the `inference` capability. Setup
//! validates that the fields form an upstream; the inference gateway then forwards to it.
use super::runtime::Runtime;
use crate::connections::{categories::CATEGORY_INFERENCE, model, service::Connections};
use crate::error::Error;
use crate::inference::Provider;

pub fn definitions() -> Vec<model::Provider> {
    Provider::ALL.into_iter().map(definition).collect()
}
fn definition(provider: Provider) -> model::Provider {
    model::Provider {
        account_name_label: Some(provider.account_name_label().into()),
        icon_url: Some(provider.icon_url()),
        instructions: Some(format!(
            "Connect {} so this agent can run inference through Tilde.",
            provider.name()
        )),
        id: provider.id().into(),
        name: provider.name().into(),
        kind: model::ProviderKind::BuiltIn,
        categories: vec![CATEGORY_INFERENCE.into()],
        connection_types: vec![model::ConnectionType {
            id: "api".into(),
            name: format!("{} API", provider.name()),
            credential_source: model::CredentialSource::Static {
                schema: provider.schema(),
            },
            capabilities: vec![model::Capability::Inference],
        }],
    }
}
pub(crate) struct Inference(Provider);
static RUNTIMES: [Inference; 11] = [
    Inference(Provider::OpenAi),
    Inference(Provider::Anthropic),
    Inference(Provider::AzureOpenAi),
    Inference(Provider::AzureAi),
    Inference(Provider::GoogleAi),
    Inference(Provider::Bedrock),
    Inference(Provider::Baseten),
    Inference(Provider::Cerebras),
    Inference(Provider::VercelAiGateway),
    Inference(Provider::Cohere),
    Inference(Provider::Voyage),
];
pub(crate) fn runtime(id: &str) -> Option<&'static dyn Runtime> {
    let provider = Provider::parse(id)?;
    RUNTIMES
        .iter()
        .find(|r| r.0 == provider)
        .map(|r| r as &dyn Runtime)
}
#[async_trait::async_trait]
impl Runtime for Inference {
    fn instructions(&self, _typ: &model::ConnectionType) -> &'static [&'static str] {
        self.0.instructions()
    }
    async fn validate(
        &self,
        _service: &Connections,
        values: &model::Values,
    ) -> Result<Option<String>, Error> {
        self.0.upstream(values)?;
        Ok(None)
    }
}
