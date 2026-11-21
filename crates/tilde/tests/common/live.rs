//! A ready connection whose every tool the agent holds (`Live`), shared by the sandbox egress test
//! and the live tool tests. Live tool tests put a real provider account behind it and make each
//! call through the agent runtime to the real upstream. Opt-in with the `live-tool-tests` feature
//! and `--ignored` (`task test:tools:live`); a missing credential fails the test rather than
//! skipping it.
#![allow(dead_code, reason = "Each live test binary uses part of this harness.")]
use super::invocation::{Fixture, all_tools};
use secrecy::SecretString;
use serde_json::Value;
use std::collections::BTreeMap;
use tilde::{
    connections::catalog::Endpoints,
    encryption::{Encryption, SecretBinding},
    tools::{Owner, Target},
};
use uuid::Uuid;

/// A required test credential or setting. Values never appear in assertion messages.
pub fn setting(name: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| panic!("{name} is required; run task secrets:load"))
}
pub fn optional(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}
/// A name no earlier run left behind, for resources a test creates and then removes.
pub fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", &Uuid::new_v4().simple().to_string()[..10])
}

pub struct Live {
    pub fx: Fixture,
    pub connection: Uuid,
    /// The prefix of the agent's tool names for this connection.
    pub slug: String,
}
impl Live {
    /// A ready `provider`/`typ` connection holding `values`, its every tool given to the agent.
    pub async fn new(seed: u8, provider: &str, typ: &str, values: &[(&str, String)]) -> Self {
        Self::with_endpoints(seed, provider, typ, values, Endpoints(BTreeMap::new())).await
    }
    pub async fn with_endpoints(
        seed: u8,
        provider: &str,
        typ: &str,
        values: &[(&str, String)],
        endpoints: Endpoints,
    ) -> Self {
        let fx = Fixture::new(seed, endpoints, all_tools()).await;
        let crypto = Encryption::initialize(&fx.db.pool, super::seed(seed))
            .await
            .unwrap();
        let (connection, slug) = fx
            .within(async {
                    let connection = Uuid::new_v4();
                    fx.connections
                        .start(connection, provider, provider, typ, &[])
                        .await
                        .unwrap();
                    let pg = fx.db.pool.get().await.unwrap();
                    for (key, value) in values {
                        let sealed = crypto
                            .seal(
                                SecretBinding {
                                    resource_kind: "connection",
                                    resource_id: connection,
                                    name: key,
                                },
                                &SecretString::from(value.clone()),
                            )
                            .unwrap()
                            .into_bytes();
                        pg.execute(
                            "INSERT INTO connection_values(connection_id,field_key,encrypted_value) VALUES($1,$2,$3)",
                            &[&connection, key, &sealed],
                        )
                        .await
                        .unwrap();
                    }
                    pg.execute(
                        "UPDATE connections SET status='ready' WHERE id=$1",
                        &[&connection],
                    )
                    .await
                    .unwrap();
                    let tools = fx.chat.tools.clone().unwrap();
                    let offered: Vec<String> = tools
                        .provider_tools(connection)
                        .await
                        .unwrap()
                        .into_iter()
                        .map(|t| t.name)
                        .collect();
                    let slug = tools
                        .add_source(
                            Owner::Agent(fx.agent),
                            Target::Connection(connection),
                            &offered,
                        )
                        .await
                        .unwrap()
                        .slug;
                (connection, slug)
            })
            .await;
        Self {
            fx,
            connection,
            slug,
        }
    }
    /// A tool's output, or its error envelope.
    pub async fn try_call(&self, tool: &str, input: Value) -> Result<Value, Value> {
        let result = self
            .fx
            .invoke(&format!("{}.{tool}", self.slug), input)
            .await;
        match result["outputJson"].as_str() {
            Some(output) => Ok(serde_json::from_str(output).unwrap()),
            None => Err(result["error"].clone()),
        }
    }
    /// A tool's output; a failed call fails the test with the provider's message.
    pub async fn call(&self, tool: &str, input: Value) -> Value {
        self.try_call(tool, input)
            .await
            .unwrap_or_else(|error| panic!("{tool} failed: {error}"))
    }
    pub async fn close(self) {
        self.fx.close().await;
    }
}
