//! Collecting what an agent's code declares.
//!
//! Prompts, skills and bundled tools are values in the agent's own modules, so only that
//! language's runtime can read them: the SDKs expose a `tilde-declarations` entrypoint that
//! imports the entry module and prints `DeploymentDeclarations` as protobuf JSON. The CLI runs
//! it and owns everything after that (uploads and registration), so registration has one
//! implementation rather than one per SDK.
use crate::project::{Project, Runtime};
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::process::Stdio;
use tokio::process::Command;

/// How to run the SDK's declaration reader. Resolved by looking for the reader itself rather
/// than by running something and interpreting its exit code, so that "the SDK is not installed"
/// stays distinguishable from "the agent's declarations are broken".
fn reader(project: &Project) -> Option<(String, Vec<String>)> {
    if let Ok(override_path) = std::env::var("TILDE_DECLARATIONS") {
        return Some((override_path, vec![]));
    }
    let installed = |relative: &str| project.dir.join(relative).is_file();
    match project.runtime {
        Runtime::Node => {
            // The linked bin of a direct @trytilde/sdk dependency, then the module itself, which
            // is present even when the package manager linked no bin.
            if installed("node_modules/.bin/tilde-declarations") {
                return Some((
                    project
                        .dir
                        .join("node_modules/.bin/tilde-declarations")
                        .to_string_lossy()
                        .into_owned(),
                    vec![],
                ));
            }
            if installed("node_modules/@trytilde/sdk/dist/cli.js") {
                return Some((
                    "node".into(),
                    vec![
                        project
                            .dir
                            .join("node_modules/@trytilde/sdk/dist/cli.js")
                            .to_string_lossy()
                            .into_owned(),
                    ],
                ));
            }
            None
        }
        Runtime::Python => {
            // An activated environment first, then the nearest `.venv`: uv keeps one virtualenv
            // at the root of a workspace, above the project that declares the agent.
            let script = |dir: &std::path::Path| {
                ["bin/tilde-declarations", "Scripts/tilde-declarations.exe"]
                    .iter()
                    .map(|relative| dir.join(relative))
                    .find(|candidate| candidate.is_file())
            };
            if let Some(active) = std::env::var_os("VIRTUAL_ENV")
                && let Some(found) = script(std::path::Path::new(&active))
            {
                return Some((found.to_string_lossy().into_owned(), vec![]));
            }
            for dir in project.dir.ancestors() {
                if let Some(found) = script(&dir.join(".venv")) {
                    return Some((found.to_string_lossy().into_owned(), vec![]));
                }
            }
            // No virtualenv to be found: fall back to whichever interpreter is on PATH, but only
            // if it can actually import the SDK.
            let importable = std::process::Command::new("python")
                .args(["-c", "import tilde"])
                .current_dir(&project.dir)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success());
            importable.then(|| {
                (
                    "python".into(),
                    vec!["-m".into(), "tilde".into(), "declarations".into()],
                )
            })
        }
    }
}

pub struct Declarations {
    pub value: Value,
    /// Stable digest of the declarations, so `tilde dev` can tell a real change from a rebuild.
    pub digest: String,
}

/// `allow_empty` is for `tilde dev`: an agent that declares no prompts, skills or tools is still
/// worth running, while an empty release is almost always a build that did not happen.
pub async fn declarations(
    project: &Project,
    entry: &str,
    allow_empty: bool,
) -> Result<Declarations> {
    let Some((program, leading)) = reader(project) else {
        bail!(
            "No Tilde declaration reader for this {} project. Install the SDK with `{}`, then try again.",
            project.runtime.label(),
            match project.runtime {
                Runtime::Node => "npm install @trytilde/sdk",
                Runtime::Python => "uv add trytilde",
            }
        );
    };
    let output = Command::new(&program)
        .args(&leading)
        .arg(entry)
        .args(allow_empty.then_some("--allow-empty"))
        .current_dir(&project.dir)
        .stdin(Stdio::null())
        // The reader writes its inventory to stderr; let the user see it as it goes.
        .stderr(Stdio::inherit())
        .output()
        .await
        .with_context(|| format!("Could not run the declaration reader ({program})"))?;
    if !output.status.success() {
        bail!("Reading the declarations in {entry} failed; see the output above");
    }
    let text = String::from_utf8(output.stdout)
        .context("The declaration reader printed output that was not UTF-8")?;
    let value: Value = serde_json::from_str(text.trim())
        .context("The declaration reader did not print declaration JSON")?;
    Ok(Declarations {
        digest: digest(&value),
        value,
    })
}

/// Canonical digest: serde_json's maps are ordered, so re-serialising is stable across runs.
fn digest(value: &Value) -> String {
    use sha2::{Digest, Sha256};
    let canonical = serde_json::to_vec(value).expect("declarations re-serialise");
    hex::encode(&Sha256::digest(&canonical)[..8])
}

/// Declarations list skills inline; files the engine may not hold yet travel as `data` (base64
/// bytes) in the reader's output and must become content-addressed uploads before registration.
pub fn pending_uploads(value: &Value) -> Vec<(String, Vec<u8>)> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use sha2::{Digest, Sha256};
    let mut uploads = Vec::new();
    for skill in value
        .get("skills")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        for file in skill
            .get("files")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(data) = file.get("data").and_then(Value::as_str)
                && let Ok(bytes) = STANDARD.decode(data)
            {
                uploads.push((hex::encode(Sha256::digest(&bytes)), bytes));
            }
        }
    }
    uploads
}

/// Replace every inline `data` with the digest the engine now holds it under.
pub fn seal_uploads(value: &mut Value) {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use sha2::{Digest, Sha256};
    let Some(skills) = value.get_mut("skills").and_then(Value::as_array_mut) else {
        return;
    };
    for skill in skills {
        let Some(files) = skill.get_mut("files").and_then(Value::as_array_mut) else {
            continue;
        };
        for file in files {
            let Some(data) = file.get("data").and_then(Value::as_str) else {
                continue;
            };
            let Ok(bytes) = STANDARD.decode(data) else {
                continue;
            };
            let sha256 = hex::encode(Sha256::digest(&bytes));
            let object = file.as_object_mut().expect("a declared file is an object");
            object.remove("data");
            object.insert("sha256".into(), Value::String(sha256));
        }
    }
}
