//! A blueprint's egress grants: which connections its sandboxes reach through the gateway, at
//! which slug, and the config narrowing the provider's rules (`connections::egress`).
use super::{Sandboxes, db};
use crate::connections::{egress, model::Capability, model::invalid};
use crate::error::Error;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Grant {
    pub slug: String,
    pub connection_id: Uuid,
    pub config: Value,
    pub config_version: u32,
    pub enabled: bool,
}
impl From<db::EgressRow> for Grant {
    fn from(row: db::EgressRow) -> Self {
        Self {
            slug: row.slug,
            connection_id: row.connection_id,
            config: row.config,
            config_version: row.config_version as u32,
            enabled: row.enabled,
        }
    }
}

fn slug(value: &str) -> Result<&str, Error> {
    let valid = value.len() <= 32
        && value.starts_with(|c: char| c.is_ascii_lowercase())
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    valid.then_some(value).ok_or_else(|| {
        invalid("An egress slug is lowercase letters, digits and underscores, starting with a letter (at most 32)")
    })
}

impl Sandboxes {
    /// The egress hook of a ready, installation-owned connection whose type offers egress.
    pub(crate) async fn egress_hook(
        &self,
        connection: Uuid,
    ) -> Result<&'static dyn egress::EgressHook, Error> {
        db::launch_connection_opt(&self.connections.pool.get().await?, connection)
            .await?
            .ok_or_else(|| invalid("Connection not found"))?;
        let row = self.connections.get(connection).await?;
        if row.status != "ready" {
            return Err(invalid("The connection is not ready"));
        }
        let provider = self.connections.provider(&row.provider_id).await?;
        let offers = provider
            .connection_types
            .iter()
            .any(|t| t.id == row.type_id && t.capabilities.contains(&Capability::Egress));
        match egress::hook(&row.provider_id) {
            Some(hook) if offers => Ok(hook),
            _ => Err(invalid(
                "Sandboxes cannot reach this connection's provider through egress",
            )),
        }
    }
    /// Adds or replaces the grant `slug`; the config is validated against the provider's schema
    /// and stored with its defaults filled in.
    pub async fn set_egress(
        &self,
        blueprint: Uuid,
        slug_value: &str,
        connection: Uuid,
        config: &Value,
        enabled: bool,
    ) -> Result<super::Blueprint, Error> {
        let slug = slug(slug_value)?;
        self.blueprint(blueprint).await?;
        let hook = self.egress_hook(connection).await?;
        let definition = hook.definition();
        let config = egress::validate_config(definition, config)?;
        let saved = db::egress_set_execute(
            &self.connections.pool.get().await?,
            blueprint,
            slug,
            connection,
            &config,
            definition.config_version as i32,
            enabled,
        )
        .await;
        match saved {
            Err(e) if e.is_unique_violation() => {
                return Err(invalid(
                    "The blueprint already grants egress to this connection under another slug",
                ));
            }
            other => other?,
        };
        self.blueprint(blueprint).await
    }
    pub async fn delete_egress(
        &self,
        blueprint: Uuid,
        slug: &str,
    ) -> Result<super::Blueprint, Error> {
        db::egress_delete_execute(&self.connections.pool.get().await?, blueprint, slug).await?;
        self.blueprint(blueprint).await
    }
}

/// The grant a request through the egress route uses.
pub struct ResolvedGrant {
    pub sandbox_id: Uuid,
    pub connection_id: Uuid,
    pub provider_id: String,
    pub type_id: String,
    pub config: Value,
    pub config_version: u32,
    pub hook: &'static dyn egress::EgressHook,
}

/// What a connecting sandbox process sets up for its grants: environment for its shells and git
/// URL rewrites to the route.
pub struct Wiring {
    pub env: Vec<(String, String)>,
    pub git: Vec<(String, String)>,
}

impl Sandboxes {
    /// The enabled grant `slug` of the sandbox whose egress token has `digest`, if its
    /// connection is still ready, installation-owned and egress-capable.
    pub(crate) async fn egress_grant(
        &self,
        digest: &[u8],
        slug: &str,
    ) -> Result<Option<ResolvedGrant>, Error> {
        let row = db::egress_grant_opt(&self.connections.pool.get().await?, digest, slug).await?;
        let Some(row) = row else { return Ok(None) };
        let provider = self.connections.provider(&row.provider_id).await?;
        let offers = provider
            .connection_types
            .iter()
            .any(|t| t.id == row.type_id && t.capabilities.contains(&Capability::Egress));
        let Some(hook) = egress::hook(&row.provider_id).filter(|_| offers) else {
            return Ok(None);
        };
        Ok(Some(ResolvedGrant {
            sandbox_id: row.sandbox_id,
            connection_id: row.connection_id,
            provider_id: row.provider_id,
            type_id: row.type_id,
            config: row.config,
            config_version: row.config_version as u32,
            hook,
        }))
    }
    /// The blueprint's enabled grants as a sandbox process wires them: `TILDE_EGRESS_<SLUG>_URL`
    /// and the provider's HTTP environment templates for each, and for git, the upstream
    /// rewritten to the route. `{token}` stays for the process to fill in.
    pub(crate) async fn egress_wiring(&self, blueprint: Uuid) -> Result<Wiring, Error> {
        let mut wiring = Wiring {
            env: vec![],
            git: vec![],
        };
        let rows = db::egress_wiring_all(&self.connections.pool.get().await?, blueprint).await?;
        for row in rows {
            let Some(hook) = egress::hook(&row.provider_id) else {
                continue;
            };
            let url = format!(
                "{}{}/{}",
                self.runtime_url.trim_end_matches('/'),
                super::gateway::PREFIX,
                row.slug
            );
            wiring.env.push((
                format!("TILDE_EGRESS_{}_URL", row.slug.to_uppercase()),
                url.clone(),
            ));
            for protocol in &hook.definition().protocols {
                match protocol {
                    egress::EgressProtocol::Http { wiring: env, .. } => {
                        for template in env {
                            wiring.env.push((
                                template.name.to_owned(),
                                template.value.replace("{url}", &url),
                            ));
                        }
                    }
                    egress::EgressProtocol::Git { upstream, .. } => {
                        wiring.git.push((format!("{url}/"), format!("{upstream}/")));
                    }
                }
            }
        }
        Ok(wiring)
    }
}
