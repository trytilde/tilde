//! Stripe refunds through the Stripe API with a secret or restricted API key.
use super::{
    ToolProvider,
    rest::{self, Hints},
};
use crate::chat::{providers::Access, tools::ToolResult};
use crate::connections::{categories::CATEGORY_PAYMENTS, model};
use crate::proto::tilde::types::v1 as types;
use connectrpc::ConnectError;
use futures::future::BoxFuture;
use serde_json::{Value, json};

pub fn definition() -> model::Provider {
    model::Provider {
        account_name_label: Some("API key name".into()),
        icon_url: Some("/provider-icons/stripe.svg".into()),
        instructions: Some(
            "Connect Stripe so agents can refund payments. Use a restricted key with write access to refunds, created under Developers > API keys."
                .into(),
        ),
        id: "stripe".into(),
        name: "Stripe".into(),
        kind: model::ProviderKind::BuiltIn,
        categories: vec![CATEGORY_PAYMENTS.into()],
        connection_types: vec![model::ConnectionType {
            mcp: None,
            id: "api".into(),
            name: "Stripe API key".into(),
            credential_source: model::CredentialSource::Static {
                schema: json!({"type":"object","properties":{"api_key":{"type":"string","title":"API key","minLength":1,"writeOnly":true}},"required":["api_key"],"additionalProperties":false}),
            },
            capabilities: vec![model::Capability::Tool],
        }],
    }
}

pub struct Stripe;

impl ToolProvider for Stripe {
    fn tools(&self) -> Vec<types::ToolDefinition> {
        vec![rest::definition(
            "stripe",
            "process_refund",
            "Refund a payment",
            "Refund a charge or PaymentIntent in full, or in part with `amount` in the smallest currency unit. Give exactly one of charge or payment_intent.",
            json!({"type":"object","properties":{
                "charge":{"type":"string","pattern":"^(ch|py)_[A-Za-z0-9]+$"},
                "payment_intent":{"type":"string","pattern":"^pi_[A-Za-z0-9]+$"},
                "amount":{"type":"integer","minimum":1},
                "reason":{"enum":["duplicate","fraudulent","requested_by_customer"]}
            },"additionalProperties":false}),
            // Money leaves the account and cannot be recalled.
            Hints {
                read_only: false,
                destructive: true,
            },
        )]
    }
    fn invoke<'a>(
        &'a self,
        access: &'a Access,
        call_id: uuid::Uuid,
        name: &'a str,
        input: Value,
    ) -> BoxFuture<'a, ToolResult<Value>> {
        Box::pin(async move {
            if name != "process_refund" {
                return Err(ConnectError::not_found("Unsupported Stripe tool"));
            }
            if input.get("charge").is_some() == input.get("payment_intent").is_some() {
                return Err(ConnectError::invalid_argument(
                    "Give exactly one of charge or payment_intent",
                ));
            }
            // Stripe takes form-encoded parameters.
            let form = {
                let mut form = url::form_urlencoded::Serializer::new(String::new());
                for key in ["charge", "payment_intent", "amount", "reason"] {
                    match &input[key] {
                        Value::String(value) => {
                            form.append_pair(key, value);
                        }
                        Value::Number(value) => {
                            form.append_pair(key, &value.to_string());
                        }
                        _ => {}
                    }
                }
                form.finish()
            };
            let url = access.url("stripe_api", "https://api.stripe.com", &["v1", "refunds"])?;
            let request = access
                .http
                .client
                .post(url)
                .bearer_auth(access.secret("api_key")?)
                // A retried call reaches Stripe as the same refund.
                .header("Idempotency-Key", call_id.to_string())
                .header("content-type", "application/x-www-form-urlencoded")
                .body(form);
            rest::send(request).await
        })
    }
}
