//! `tilde build`: an agent's deployable artifacts, built exactly as `tilde deploy` builds them,
//! written to a folder instead of being deployed. It contacts no Tilde server and needs no
//! credentials, so CI can publish a prebuilt agent (Tilde Cloud's Factory Manager is one).
//!
//! The agent is chosen as `tilde deploy` finds them: the entry's `@trytilde/iac` specs (one, or
//! `--agent <key>`), each agent's code read from `agents/<key>/` or its `code`
//! (`discover::releases`); an entry without specs is one agent whose target is `--target`.
//! The folder gets:
//!
//! - `declarations.json`: the code's `DeploymentDeclarations` as protobuf JSON, as the SDK's
//!   reader prints them (skill files the engine may not hold yet still inline as `data`);
//! - `agent.json`: the agent's spec as `tilde deploy`'s Apply sends it (`AgentSpec` protobuf JSON,
//!   its deployment carrying those declarations and its external ID). Not for an entry without
//!   specs: it has none;
//! - `bundle.zip`: a Hosted agent's bundle (`hosted::bundle`), reproducible from its sources;
//! - `manifest.json`, with `--version`: `{ version, sha256, declarations }`, the zip's digest
//!   beside its declarations, which is what Tilde Cloud installs a published bundle from.
use crate::{apply, deploy::Deploy, discover, project::Project};
use anyhow::{Context, Result, bail};
use clap::Args;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Args)]
#[command(disable_version_flag = true)]
pub struct Build {
    /// Module declaring the agent: the project's entry point by default (its tilde.config.ts).
    pub entry: Option<String>,
    /// The agent to build, by key: needed when the specs declare more than one.
    #[arg(long)]
    pub agent: Option<String>,
    /// The folder to write the artifacts to. Files of an earlier build there are replaced.
    #[arg(long, short, default_value = "tilde-build")]
    pub out: PathBuf,
    /// The published version (`1.4.0`): also writes manifest.json. A Hosted agent only.
    #[arg(long)]
    pub version: Option<String>,
    /// Without specs: the agent's deploy target, as for `tilde deploy` (default gateway). A
    /// spec's `deploy` names its own.
    #[arg(long, value_parser = ["gateway", "sidecar", "lambda", "hosted"])]
    pub target: Option<String>,
    #[arg(long, default_value = ".", hide = true)]
    pub dir: PathBuf,
}

/// The files a build writes; any of them left from an earlier build is removed first.
const OUTPUTS: [&str; 4] = [
    "bundle.zip",
    "declarations.json",
    "agent.json",
    "manifest.json",
];

struct Built {
    /// The agent's spec as Apply would receive it; none for an entry without specs.
    agent: Option<Value>,
    /// The released code's declarations; none for an agent whose code is elsewhere.
    declarations: Option<Value>,
    bundle: Option<Vec<u8>>,
}

#[derive(serde::Serialize)]
struct Manifest<'a> {
    version: &'a str,
    sha256: &'a str,
    declarations: &'a Value,
}

pub async fn run(args: Build) -> Result<()> {
    if let Some(version) = &args.version
        && !regex::Regex::new(r"^\d+\.\d+\.\d+([-.][0-9A-Za-z.-]+)?$")?.is_match(version)
    {
        bail!("--version {version} is not a version like 1.4.0");
    }
    let project = Project::find(&args.dir)?;
    let entry = match &args.entry {
        Some(entry) => entry.clone(),
        None => project.entry()?,
    };
    let mut declarations = discover::declarations(&project, &entry, false, true).await?;
    let built = if declarations.agents.is_empty() {
        without_specs(&args, &project, &entry, declarations)?
    } else {
        discover::releases(&project, &entry, &mut declarations).await?;
        from_spec(&args, &declarations)?
    };
    if args.version.is_some() && built.bundle.is_none() {
        bail!("--version writes a Hosted agent's manifest, and this agent builds no bundle");
    }
    write(&args.out, args.version.as_deref(), &built)
}

fn without_specs(
    args: &Build,
    project: &Project,
    entry: &str,
    declarations: discover::Declarations,
) -> Result<Built> {
    if let Some(key) = &args.agent {
        bail!("--agent {key}: {entry} exports no @trytilde/iac agent specs");
    }
    let hosted = args.target.as_deref() == Some("hosted");
    Ok(Built {
        agent: None,
        bundle: hosted
            .then(|| crate::hosted::bundle(&project.dir, entry))
            .transpose()?,
        declarations: Some(declarations.value),
    })
}

fn from_spec(args: &Build, declarations: &discover::Declarations) -> Result<Built> {
    if args.target.is_some() {
        bail!("--target does not apply to @trytilde/iac specs: the agent's spec names its target");
    }
    let keys: Vec<&str> = declarations
        .agents
        .iter()
        .filter_map(|agent| agent["key"].as_str())
        .collect();
    let key = match (&args.agent, &keys[..]) {
        (Some(key), _) if keys.contains(&key.as_str()) => key.as_str(),
        (Some(key), _) => bail!("No agent {key}: the specs declare {}", keys.join(", ")),
        (None, [key]) => key,
        (None, _) => bail!(
            "The specs declare agents {}: choose one with --agent <key>",
            keys.join(", ")
        ),
    };
    let spec = declarations
        .agents
        .iter()
        .find(|agent| agent["key"] == key)
        .context("the chosen agent is declared")?;
    let mut bundles = apply::Bundles::new();
    if spec["deployment"]["target"] == apply::HOSTED
        && let Some((dir, entry)) = declarations.sources.get(key)
    {
        eprintln!("Agent {key}: bundling {entry} for Tilde to host");
        bundles.insert(key.to_owned(), crate::hosted::bundle(dir, entry)?);
    }
    // The request `tilde deploy` would send, of which this agent is one: its deployment carries
    // the release's declarations and an external ID with the bundle's digest in it.
    let request = apply::request_with(&Deploy::default(), declarations, &bundles)?;
    let agent = request
        .agents
        .iter()
        .find(|agent| agent.key == key)
        .context("the chosen agent is in the request")?;
    Ok(Built {
        agent: Some(serde_json::to_value(agent)?),
        declarations: declarations.releases.get(key).cloned(),
        bundle: bundles.remove(key),
    })
}

fn write(out: &Path, version: Option<&str>, built: &Built) -> Result<()> {
    std::fs::create_dir_all(out).with_context(|| format!("Creating {}", out.display()))?;
    for name in OUTPUTS {
        match std::fs::remove_file(out.join(name)) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error)?,
            _ => {}
        }
    }
    let mut written = Vec::new();
    if let Some(declarations) = &built.declarations {
        write_json(out, "declarations.json", declarations)?;
        let count = |kind: &str| declarations[kind].as_array().map_or(0, Vec::len);
        written.push(format!(
            "declarations.json  {} prompt(s), {} skill(s), {} tool(s)",
            count("prompts"),
            count("skills"),
            count("tools")
        ));
    }
    if let Some(agent) = &built.agent {
        write_json(out, "agent.json", agent)?;
        written.push(format!(
            "agent.json         {}, {}",
            agent["key"].as_str().unwrap_or_default(),
            agent["deployment"]["target"]
                .as_str()
                .map_or("no deployment from this code".into(), |t| t
                    .trim_start_matches("DEPLOYMENT_TARGET_")
                    .to_lowercase())
        ));
    }
    match &built.bundle {
        Some(bundle) => {
            std::fs::write(out.join("bundle.zip"), bundle).context("Writing bundle.zip")?;
            let sha256 = hex::encode(Sha256::digest(bundle));
            written.push(format!(
                "bundle.zip         {} KiB, sha256 {sha256}",
                bundle.len().div_ceil(1024)
            ));
            if let (Some(version), Some(declarations)) = (version, &built.declarations) {
                let manifest = Manifest {
                    version,
                    sha256: &sha256,
                    declarations,
                };
                write_json(out, "manifest.json", &manifest)?;
                written.push(format!("manifest.json      version {version}"));
            }
        }
        None => written.push("No bundle: only a Hosted agent's code is bundled".into()),
    }
    eprintln!("Built into {}:", out.display());
    for line in written {
        eprintln!("  {line}");
    }
    Ok(())
}

fn write_json(out: &Path, name: &str, value: &impl serde::Serialize) -> Result<()> {
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    std::fs::write(out.join(name), text).with_context(|| format!("Writing {name}"))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use axum::{Router, body::Bytes, http::header, response::IntoResponse, routing::post};
    use buffa::Message;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use tilde_contracts::proto::tilde::management::v1 as wire;

    fn write(root: &Path, path: &str, content: &str) {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    /// A project whose tilde.config.ts declares a Hosted agent (`manager`) and a gateway one
    /// (`helper`), each with its code in `agents/<key>/`. The declaration reader stands in for the
    /// SDK's (specs when asked for them, else the code's prompt and skill, named after its
    /// folder), and esbuild for the real one (the "bundle" is the entry as it is).
    fn fixture() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let r = root.path();
        write(r, "package.json", r#"{"name": "factory"}"#);
        write(r, "tilde.config.ts", "");
        for key in ["manager", "helper"] {
            write(
                r,
                &format!("agents/{key}/package.json"),
                r#"{"name": "agent", "main": "index.js"}"#,
            );
            write(
                r,
                &format!("agents/{key}/index.js"),
                "export const handler = 1;",
            );
        }
        let m = r.join("agents/manager");
        write(&m, "prompts/instructions.md", "Run the factory.");
        write(
            &m,
            "skills/seed-the-wiki/SKILL.md",
            "---\nname: seed-the-wiki\n---\nSeed it.",
        );
        write(
            &m,
            "node_modules/@trytilde/sdk/package.json",
            r#"{"name": "@trytilde/sdk", "version": "9.9.9"}"#,
        );
        write(
            &m,
            "node_modules/esbuild/lib/main.js",
            "import { cpSync, mkdirSync } from 'node:fs'; import { dirname } from 'node:path';\n\
             export async function build(o) { mkdirSync(dirname(o.outfile), { recursive: true }); \
             cpSync(o.entryPoints[0], o.outfile); \
             return { metafile: { outputs: { [o.outfile]: { exports: ['handler'] } } } }; }\n",
        );
        let specs = json!({
            "declarations": {"prompts": [], "skills": [], "tools": []},
            "agents": [
                {"key": "manager", "name": "Factory Manager",
                 "deployment": {"target": "DEPLOYMENT_TARGET_HOSTED"},
                 "sandbox": {"name": "factory", "connection": {"slug": "tilde_sandbox/sandbox"}, "template": "debian",
                             "egress": [{"slug": "github", "connection": {"slug": "github/acme"}, "enabled": true}]},
                 "tools": [{"connection": {"slug": "linear/acme"}, "tools": ["get_issue"]}],
                 "inference": [{"alias": "default", "connection": {"slug": "anthropic/acme"}}],
                 "routines": [{"name": "daily", "prompt": "Check in.", "trigger": {"cron": {"schedule": "0 9 * * *"}}}]},
                {"key": "helper", "name": "Helper", "deployment": {"target": "DEPLOYMENT_TARGET_GATEWAY"}}
            ]
        });
        write(
            r,
            "reader.sh",
            &format!(
                "#!/bin/sh\nif [ \"$TILDE_DISCOVERY_SPECS\" = 1 ]; then echo '{specs}'; exit; fi\n\
                 echo '{{\"prompts\": [{{\"name\": \"'\"$(basename \"$PWD\")\"'\", \"template\": \"Run the factory.\"}}], \
                 \"skills\": [{{\"name\": \"seed-the-wiki\", \"files\": [{{\"path\": \"SKILL.md\", \"content\": \"Seed it.\"}}]}}], \
                 \"tools\": []}}'\n"
            ),
        );
        std::fs::set_permissions(
            r.join("reader.sh"),
            std::os::unix::fs::PermissionsExt::from_mode(0o755),
        )
        .unwrap();
        root
    }

    async fn build(
        root: &Path,
        agent: Option<&str>,
        out: &str,
        version: Option<&str>,
    ) -> Result<()> {
        let args = Build {
            entry: None,
            agent: agent.map(str::to_owned),
            out: root.join(out),
            version: version.map(str::to_owned),
            target: None,
            dir: root.to_path_buf(),
        };
        run(args).await
    }
    fn read(path: PathBuf) -> Value {
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }

    /// A Hosted agent builds the same bundle twice (same digest), with the layout the SDK's
    /// handler runs from and a manifest of its digest and declarations; its agent.json is the
    /// agent of the request `tilde deploy` then sends, and of a dry run but for the external ID
    /// (a dry run builds no bundle, so its ID has no bundle digest in it). A gateway agent builds
    /// its declarations and spec and no bundle.
    #[tokio::test]
    async fn a_hosted_agent_builds_what_deploy_would_send() {
        let root = fixture();
        let r = root.path();
        // SAFETY: tests in this binary do not read these variables concurrently.
        unsafe {
            std::env::set_var("TILDE_DECLARATIONS", r.join("reader.sh"));
            for name in ["GITHUB_RUN_ID", "GITHUB_REPOSITORY", "GITHUB_SHA"] {
                std::env::remove_var(name);
            }
        }
        let error = build(r, None, "out", None).await.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("agents manager, helper: choose one"),
            "{error}"
        );

        build(r, Some("manager"), "a", Some("1.4.0")).await.unwrap();
        build(r, Some("manager"), "b", Some("1.4.0")).await.unwrap();
        let zip = std::fs::read(r.join("a/bundle.zip")).unwrap();
        assert_eq!(zip, std::fs::read(r.join("b/bundle.zip")).unwrap());
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip.clone())).unwrap();
        let names: Vec<&str> = archive.file_names().collect();
        assert_eq!(
            names,
            [
                "dist/index.mjs",
                "prompts/instructions.md",
                "skills/seed-the-wiki/SKILL.md",
                "package.json"
            ]
        );
        let mut package = String::new();
        std::io::Read::read_to_string(&mut archive.by_name("package.json").unwrap(), &mut package)
            .unwrap();
        assert_eq!(package, r#"{"private":true,"version":"9.9.9"}"#);

        let declarations = read(r.join("a/declarations.json"));
        assert_eq!(declarations["prompts"][0]["name"], "manager");
        let manifest = std::fs::read_to_string(r.join("a/manifest.json")).unwrap();
        // Tilde Cloud's install reads exactly these keys, in this order.
        assert!(
            manifest.starts_with("{\n  \"version\": \"1.4.0\",\n  \"sha256\": \""),
            "{manifest}"
        );
        let manifest: Value = serde_json::from_str(&manifest).unwrap();
        assert_eq!(manifest["sha256"], hex::encode(Sha256::digest(&zip)));
        assert_eq!(manifest["declarations"], declarations);

        // What `tilde deploy` sends for the same project, to a stand-in server.
        let received: Arc<Mutex<Vec<wire::ApplyRequest>>> = Arc::default();
        let app = Router::new().route(
            "/tilde.management.v1.ApplyService/Apply",
            post({
                let received = received.clone();
                move |body: Bytes| async move {
                    let request = wire::ApplyRequest::decode_from_slice(&body).unwrap();
                    let outcome = match request.dry_run {
                        true => wire::ApplyOutcome::Planned,
                        false => wire::ApplyOutcome::Applied,
                    };
                    received.lock().unwrap().push(request);
                    let response = wire::ApplyResponse {
                        outcome: outcome.into(),
                        ..Default::default()
                    };
                    (
                        [(header::CONTENT_TYPE, "application/proto")],
                        response.encode_to_vec(),
                    )
                        .into_response()
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        for dry_run in [false, true] {
            let args = Deploy {
                url: Some(url.clone()),
                dry_run,
                dir: r.to_path_buf(),
                ..Default::default()
            };
            crate::deploy::run(args).await.unwrap();
        }
        let sent = |i: usize| {
            let request = &received.lock().unwrap()[i];
            serde_json::to_value(request.agents.iter().find(|a| a.key == "manager").unwrap())
                .unwrap()
        };
        let agent = read(r.join("a/agent.json"));
        assert_eq!(agent, sent(0));
        assert_eq!(agent["sandbox"]["egress"][0]["slug"], "github");
        assert_eq!(agent["routines"][0]["name"], "daily");
        let mut dry = sent(1);
        assert_ne!(
            dry["deployment"]["externalId"],
            agent["deployment"]["externalId"]
        );
        dry["deployment"]["externalId"] = agent["deployment"]["externalId"].clone();
        assert_eq!(agent, dry);
        // The deployment's declarations are declarations.json.
        let wire_declarations: wire::DeploymentDeclarations =
            serde_json::from_value(declarations).unwrap();
        assert_eq!(
            agent["deployment"]["declarations"],
            serde_json::to_value(wire_declarations).unwrap()
        );

        // A gateway agent: declarations and spec, no bundle; an earlier build's files go.
        build(r, Some("helper"), "a", None).await.unwrap();
        assert!(!r.join("a/bundle.zip").exists() && !r.join("a/manifest.json").exists());
        assert_eq!(
            read(r.join("a/declarations.json"))["prompts"][0]["name"],
            "helper"
        );
        assert_eq!(
            read(r.join("a/agent.json"))["deployment"]["target"],
            "DEPLOYMENT_TARGET_GATEWAY"
        );
        let error = build(r, Some("helper"), "a", Some("1.4.0"))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("builds no bundle"), "{error}");
    }
}
