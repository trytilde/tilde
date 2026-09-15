//! Local SDK example bootstrap. This command is absent from release builds.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use envconfig::Envconfig;
use rand::RngCore;
use secrecy::{ExposeSecret, SecretString};
use sqlx::{PgPool, types::Json};
use std::{collections::BTreeMap, sync::Arc};
use tilde::{
    agent::{Agents, CreateAgent, UpdateAgent},
    config::SecretEnv,
    encryption::{Encryption, SealedSecret, SecretBinding},
    iam::capabilities::{Capabilities, Capability, Reach},
};
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Envconfig)]
struct Config {
    #[envconfig(from = "OPENAI_API_KEY")]
    openai_api_key: SecretEnv,
    #[envconfig(from = "DEV_AGENT_PORT", default = "3001")]
    port: u16,
    #[envconfig(from = "OPENAI_MODEL", default = "gpt-4o-mini")]
    model: String,
}

pub async fn run(
    pool: PgPool,
    encryption: Arc<Encryption>,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::init_from_env()?;
    if config.port == 0 {
        return Err("DEV_AGENT_PORT must be nonzero".into());
    }
    let id = Uuid::parse_str("9a97ef65-4f0c-4ce8-9d95-7b8aa39bca01")?;
    let endpoint = format!("http://127.0.0.1:{}", config.port);
    let agents = Agents::new(pool.clone(), encryption.clone());
    let current = sqlx::query_file!("../../queries/agent/get_for_create.sql", id)
        .fetch_optional(&pool)
        .await?;
    let key = if let Some(row) = current {
        let key = encryption.open(
            SecretBinding {
                resource_kind: "agent",
                resource_id: id,
                name: "webhook_signing_key",
            },
            SealedSecret::from_bytes(&row.webhook_signing_key)?,
        )?;
        agents
            .update(UpdateAgent {
                concurrency_policy: None,
                id,
                name: None,
                endpoint_url: Some(endpoint.clone()),
                capabilities: None,
            })
            .await?;
        key
    } else {
        let mut bytes = Zeroizing::new([0u8; 32]);
        rand::rngs::OsRng.fill_bytes(&mut *bytes);
        let key = SecretString::from(STANDARD.encode(bytes.as_slice()));
        agents
            .create(CreateAgent {
                concurrency_policy: Default::default(),
                id,
                name: "Example Agent 1".into(),
                endpoint_url: endpoint.clone(),
                webhook_signing_key: key.clone(),
                capabilities: Capabilities(BTreeMap::from([
                    (Capability::ThreadRead, Reach::Yes),
                    (Capability::RunUpdate, Reach::Yes),
                    (Capability::ToolsInvoke, Reach::All),
                ])),
            })
            .await?;
        key
    };
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../dev/example-agent-1/dist/index.js");
    let mut command = tokio::process::Command::new("node");
    command
        .arg(script)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("AGENT_SIGNING_KEY", key.expose_secret())
        .env("OPENAI_API_KEY", config.openai_api_key.0.expose_secret())
        .env("OPENAI_MODEL", config.model)
        .env("DEV_AGENT_PORT", config.port.to_string())
        .kill_on_drop(true);
    let mut child = command.spawn()?;
    drop(command);
    drop(key); // Decrypted signing material zeroizes after transfer to the agent process.
    drop(config.openai_api_key);
    println!("Registered Example Agent 1 ({id}) at {endpoint}");
    let result = tokio::select! {
        result = child.wait() => result.map(|status| if status.success() {Ok(())} else {Err("Example agent exited unsuccessfully")})?,
        _ = super::shutdown() => { child.kill().await?; Ok(()) },
    };
    pool.close().await;
    result.map_err(Into::into)
}
