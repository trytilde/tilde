//! Hosted deployments (Tilde Cloud runs the agent's code on its own Lambda): the bundle `tilde
//! deploy` builds from an agent's code and publishes once the deployment is registered.
//!
//! The bundle is the layout the SDK's Lambda handler runs from: the entry module bundled by
//! esbuild into one ES module at `dist/index.mjs`, which must export `handler`
//! (`export const handler = createLambdaHandler({ run })`), with text imports of prompts inlined
//! and imports of the project's own files (`../../connections.ts`) followed wherever they are;
//! `prompts/` and `skills/` copied beside it for code that reads them from disk; and a
//! `package.json` carrying the SDK's version, which the SDK reads from beside its module. esbuild
//! is the project's own, else a pinned one installed for the build. Node.js code only.
use crate::api::Client;
use anyhow::{Context, Result, bail};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use tilde_contracts::proto::tilde::{management::v1 as wire, types::v1 as types};

const ESBUILD: &str = "esbuild@0.25.12";
const BUILD: &str = include_str!("hosted_build.mjs");
/// What Tilde Cloud takes in one publish.
const MAX_BUNDLE_BYTES: usize = 30 * 1024 * 1024;
/// CommonJS dependencies inside an ES module bundle need `require`.
const BANNER: &str = "import { createRequire as __tildeRequire } from 'node:module'; const require = __tildeRequire(import.meta.url);";

/// The zipped bundle of the code in `dir` whose entry module is `entry`.
pub fn bundle(dir: &Path, entry: &str) -> Result<Vec<u8>> {
    if entry.ends_with(".py") {
        bail!("Hosted deployments run Node.js code; {entry} is Python");
    }
    let out = tempfile::tempdir().context("A scratch directory for the bundle")?;
    let module = out.path().join("dist/index.mjs");
    let esbuild = esbuild(dir, out.path())?;
    let script = out.path().join("build.mjs");
    std::fs::write(&script, BUILD)?;
    let output = Command::new("node")
        .current_dir(dir)
        .arg(&script)
        .arg(&esbuild)
        .arg(entry)
        .arg(&module)
        .arg(BANNER)
        .stderr(std::process::Stdio::inherit())
        .output()
        .context("Running esbuild needs Node.js")?;
    if !output.status.success() {
        bail!("esbuild could not bundle {entry}");
    }
    let exports: Vec<String> =
        serde_json::from_slice(&output.stdout).context("esbuild reported no exports")?;
    if !exports.iter().any(|name| name == "handler") {
        bail!(
            "{entry} exports no `handler`: a Hosted agent's module exports \
             `export const handler = createLambdaHandler({{ run }})`"
        );
    }
    let mut files = vec![("dist/index.mjs".to_owned(), module)];
    for folder in ["prompts", "skills"] {
        files.extend(walk(&dir.join(folder), folder)?);
    }
    let manifest = serde_json::json!({"private": true, "version": sdk_version(dir).unwrap_or_else(|| "0.0.0".into())});
    let manifest_path = out.path().join("package.json");
    std::fs::write(&manifest_path, serde_json::to_vec(&manifest)?)?;
    files.push(("package.json".into(), manifest_path));
    zip(&files)
}

/// The files under `root` as `(path in the bundle, file)`, or none when it does not exist.
fn walk(root: &Path, prefix: &str) -> Result<Vec<(String, PathBuf)>> {
    let mut files = Vec::new();
    if !root.is_dir() {
        return Ok(files);
    }
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let name = format!("{prefix}/{}", entry.file_name().to_string_lossy());
        let kind = entry.file_type()?;
        if kind.is_symlink()
            && std::fs::metadata(entry.path())
                .with_context(|| format!("{name} is a broken link"))?
                .is_dir()
        {
            // A linked folder could loop back on itself.
            bail!("{name} links to a folder; a Hosted bundle takes linked files only");
        }
        if kind.is_dir() {
            files.extend(walk(&entry.path(), &name)?);
        } else {
            // A linked file is bundled with the content it points to.
            files.push((name, entry.path()));
        }
    }
    Ok(files)
}

fn zip(files: &[(String, PathBuf)]) -> Result<Vec<u8>> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);
    for (name, path) in files {
        writer.start_file(name.as_str(), options)?;
        writer.write_all(&std::fs::read(path)?)?;
    }
    let bytes = writer.finish()?.into_inner();
    if bytes.len() > MAX_BUNDLE_BYTES {
        bail!(
            "The bundle is {} MiB; a Hosted deployment's is at most {} MiB",
            bytes.len() / 1024 / 1024,
            MAX_BUNDLE_BYTES / 1024 / 1024
        );
    }
    Ok(bytes)
}

/// esbuild's API module: the project's own (`node_modules/esbuild` in `dir` or above, as Node
/// would resolve it), else a pinned one installed into `scratch`.
fn esbuild(dir: &Path, scratch: &Path) -> Result<PathBuf> {
    let main = |root: &Path| root.join("node_modules/esbuild/lib/main.js");
    if let Some(found) = dir.ancestors().map(main).find(|p| p.is_file()) {
        return Ok(found);
    }
    let prefix = scratch.join("esbuild");
    std::fs::create_dir_all(&prefix)?;
    std::fs::write(prefix.join("package.json"), "{\"private\":true}")?;
    let status = Command::new("npm")
        .args(["install", "--silent", "--no-audit", "--no-fund", "--prefix"])
        .arg(&prefix)
        .arg(ESBUILD)
        .status()
        .context("Installing esbuild needs npm (or add esbuild to the project)")?;
    if !status.success() || !main(&prefix).is_file() {
        bail!("Could not install {ESBUILD}");
    }
    Ok(main(&prefix))
}
/// The installed `@trytilde/sdk`'s version, as Node would resolve it from `dir`.
fn sdk_version(dir: &Path) -> Option<String> {
    let manifest = dir
        .ancestors()
        .map(|d| d.join("node_modules/@trytilde/sdk/package.json"))
        .find(|p| p.is_file())?;
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(manifest).ok()?).ok()?;
    value["version"].as_str().map(str::to_owned)
}

/// Publish a registered Hosted deployment's bundle. Tilde creates or updates the agent's function
/// and publishes a version, which can take a minute.
pub async fn publish(
    client: &Client,
    agent_id: &str,
    deployment_id: &str,
    bundle: Vec<u8>,
) -> Result<types::AgentDeployment> {
    let options = crate::api::options().with_timeout(std::time::Duration::from_secs(600));
    let response = client
        .deployments
        .publish_hosted_deployment_with_options(
            wire::PublishHostedDeploymentRequest {
                agent_id: agent_id.into(),
                deployment_id: deployment_id.into(),
                bundle,
                ..Default::default()
            },
            options,
        )
        .await
        .map_err(|error| crate::api::failed("PublishHostedDeployment", error))?
        .into_owned();
    response
        .deployment
        .into_option()
        .context("PublishHostedDeployment returned no deployment")
}

/// The function version a published deployment serves (the reference's last segment).
pub fn version(deployment: &types::AgentDeployment) -> &str {
    deployment
        .target_reference
        .rsplit(':')
        .next()
        .unwrap_or_default()
}
