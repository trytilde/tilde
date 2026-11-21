//! Declarative apply of a sandbox blueprint (see `crate::apply`): found by name, its settings
//! converge on the spec and the variables it names are set (others stay; `delete` removes one); its egress grants are upserted by slug and removed only
//! with `delete`. Environment values are compared sealed and never reported.
use super::{Timings, db, env_binding, link_agent, reuse, rpc::reuse_text, text};
use crate::apply::{Change, Refs};
use crate::connections::egress;
use crate::database::Transaction;
use crate::encryption::{Encryption, SealedSecret};
use crate::error::Error;
use crate::proto::tilde::management::v1 as m;
use secrecy::{ExposeSecret, SecretString};
use uuid::Uuid;

fn timings(spec: &m::BlueprintSpec) -> Timings {
    spec.timings.as_option().map_or(
        Timings {
            sleep_after: 0,
            terminate_after: 0,
            connect_timeout: 0,
        },
        |t| Timings {
            sleep_after: t.sleep_after_seconds,
            terminate_after: t.terminate_after_seconds,
            connect_timeout: t.connect_timeout_seconds,
        },
    )
}
fn timing_text(t: Timings) -> String {
    format!(
        "sleep {}s, terminate {}s, connect {}s",
        t.sleep_after, t.terminate_after, t.connect_timeout
    )
}

pub(crate) async fn blueprint(
    tx: &Transaction<'_>,
    crypto: &Encryption,
    refs: &Refs,
    spec: &m::BlueprintSpec,
) -> Result<(Uuid, Vec<Change>), Error> {
    let name = text(&spec.name, "name", 128)?;
    let template = text(&spec.template, "template", 512)?;
    let reuse_mode = reuse(reuse_text(spec.reuse)?)?;
    let provider = refs.connection(&spec.provider)?;
    let (id, mut change) = match db::blueprint_by_name_opt(tx, &name).await? {
        Some(row) => {
            let current = Timings {
                sleep_after: row.sleep_after_secs as u32,
                terminate_after: row.terminate_after_secs as u32,
                connect_timeout: row.connect_timeout_secs as u32,
            };
            // Absent or zero timings keep the blueprint's own.
            let timings = current.with(timings(spec))?;
            let mut change = Change::existing("sandbox_blueprint", &name);
            change.field("provider", &row.connection_slug, &provider.slug);
            change.field("template", &row.template, &template);
            change.field("reuse", &row.reuse, reuse_mode);
            change.field("timings", timing_text(current), timing_text(timings));
            if change.changed() {
                db::blueprint_update_execute(
                    tx,
                    row.id,
                    None,
                    Some(provider.id),
                    Some(&template),
                    Some(reuse_mode),
                    Some(timings),
                )
                .await?;
            }
            (row.id, change)
        }
        None => {
            let timings = Timings::default().with(timings(spec))?;
            let id = Uuid::new_v4();
            db::blueprint_insert_execute(
                tx,
                id,
                &name,
                provider.id,
                &template,
                reuse_mode,
                timings,
            )
            .await?;
            let mut change = Change::create("sandbox_blueprint", &name);
            change.field("provider", "", &provider.slug);
            change.field("template", "", &template);
            (id, change)
        }
    };
    let existing = db::env_all(tx, id).await?;
    let mut names: Vec<&String> = spec.env.keys().collect();
    names.sort();
    for env_name in names {
        if spec.env[env_name].delete {
            if existing.iter().any(|r| &r.name == env_name) {
                db::env_delete_execute(tx, id, env_name).await?;
                change.field(format!("env.{env_name}"), "set", "deleted");
            }
            continue;
        }
        let current = existing.iter().find(|r| &r.name == env_name);
        let literal = match &spec.env[env_name].value {
            Some(m::env_value::Value::Literal(literal)) => literal,
            Some(m::env_value::Value::Secret(reference)) => {
                // A reference is a name, not a secret: the plan shows it.
                let secret = refs.secret(&spec.name, &reference.name)?;
                let before = match current {
                    Some(row) if row.secret_id == Some(secret) => continue,
                    Some(row) if row.secret_id.is_some() => "another secret",
                    Some(_) => "a value",
                    None => "",
                };
                db::env_set_secret_execute(tx, id, env_name, secret).await?;
                change.field(
                    format!("env.{env_name}"),
                    before,
                    format!("secret {}", reference.name),
                );
                continue;
            }
            None => {
                return Err(Error::Invalid(format!(
                    "Environment variable {env_name} has no value"
                )));
            }
        };
        let value = SecretString::from(literal.clone());
        let binding = || env_binding(id, env_name);
        let same = match current.and_then(|row| row.encrypted_value.as_deref()) {
            Some(sealed) => {
                let current = crypto.open(binding(), SealedSecret::from_bytes(sealed)?)?;
                current.expose_secret() == value.expose_secret()
            }
            None => false,
        };
        if !same {
            let sealed = crypto.seal(binding(), &value)?.into_bytes();
            db::env_set_execute(tx, id, env_name, &sealed).await?;
        }
        change.sealed(format!("env.{env_name}"), !same);
    }
    let mut changes = vec![change];
    let grants = db::egress_list_all(tx, &[id]).await?;
    for spec in &spec.egress {
        let key = format!("{name}/{}", spec.slug);
        let current = grants.iter().find(|g| g.slug == spec.slug);
        if spec.delete {
            if current.is_some() {
                db::egress_delete_execute(tx, id, &spec.slug).await?;
                changes.push(Change::remove("egress", key));
            }
            continue;
        }
        let connection = refs.connection(&spec.connection)?;
        let hook = egress::hook(&connection.provider_id)
            .ok_or_else(|| Error::Invalid(format!("{} offers no egress", connection.slug)))?;
        let raw: serde_json::Value = serde_json::from_str(&spec.config_json)
            .map_err(|_| Error::Invalid(format!("Egress {key}: config is not JSON")))?;
        let config = egress::validate_config(hook.definition(), &raw)?;
        let version = hook.definition().config_version as i32;
        let change = match current {
            Some(grant) => {
                let mut change = Change::existing("egress", key);
                if grant.connection_id != connection.id {
                    change.field("connection", grant.connection_id, &connection.slug);
                }
                if grant.config != config || grant.config_version != version {
                    change.field("config", &grant.config, &config);
                }
                change.field("enabled", grant.enabled, spec.enabled);
                change
            }
            None => {
                let mut change = Change::create("egress", key);
                change.field("connection", "", &connection.slug);
                change.field("enabled", "", spec.enabled);
                change
            }
        };
        if change.changed() {
            db::egress_set_execute(
                tx,
                id,
                &spec.slug,
                connection.id,
                &config,
                version,
                spec.enabled,
            )
            .await?;
        }
        changes.push(change);
    }
    Ok((id, changes))
}

pub(crate) async fn agent_sandbox(
    tx: &Transaction<'_>,
    agent: Uuid,
    agent_key: &str,
    blueprint_name: &str,
    blueprint: Uuid,
) -> Result<Change, Error> {
    let current = db::agent_sandbox_get_opt(tx, agent).await?;
    let change = match &current {
        Some(row) if row.blueprint_id == blueprint && row.source_id.is_some() => {
            return Ok(Change::existing("agent_sandbox", agent_key));
        }
        Some(row) => {
            let mut change = Change::existing("agent_sandbox", agent_key);
            change.field("blueprint", row.blueprint_id, blueprint_name);
            change
        }
        None => {
            let mut change = Change::create("agent_sandbox", agent_key);
            change.field("blueprint", "", blueprint_name);
            change
        }
    };
    link_agent(tx, agent, blueprint).await?;
    Ok(change)
}
