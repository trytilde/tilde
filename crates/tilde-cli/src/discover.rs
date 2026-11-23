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

#[derive(Default)]
pub struct Declarations {
    pub value: Value,
    /// Stable digest of the declarations, so `tilde dev` can tell a real change from a rebuild.
    pub digest: String,
    /// `@trytilde/iac` agent specs (`AgentSpec` protobuf JSON) the entry exports, when asked for.
    pub agents: Vec<Value>,
    /// The team a `defineProject` or `defineTeam` names.
    pub team: Option<String>,
    /// A `defineTeam`'s people (`UserSpec`/`GroupSpec` protobuf JSON).
    pub users: Vec<Value>,
    pub groups: Vec<Value>,
    /// A `defineProject`'s ontologies (Tilde Cloud), by name.
    pub ontologies: std::collections::BTreeMap<String, crate::ontology::OntologySpec>,
    /// Each agent's `defineAgent({ code })`, by key: where its code is, from the project's
    /// directory. Never sent: Apply's specs have no such field.
    pub code: std::collections::BTreeMap<String, String>,
    /// The declarations of each deploying agent's own code (`releases`), by key.
    pub releases: std::collections::BTreeMap<String, Value>,
    /// Where each released agent's code is: its folder and entry module, by key. A Hosted
    /// deployment's bundle is built from it.
    pub sources: std::collections::BTreeMap<String, (std::path::PathBuf, String)>,
}

/// `allow_empty` is for `tilde dev`: an agent that declares no prompts, skills or tools is still
/// worth running, while an empty release is almost always a build that did not happen.
/// `specs` asks the reader for `@trytilde/iac` specs too (`TILDE_DISCOVERY_SPECS`); readers that
/// predate them ignore it and print plain declarations.
pub async fn declarations(
    project: &Project,
    entry: &str,
    allow_empty: bool,
    specs: bool,
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
        .envs(specs.then_some(("TILDE_DISCOVERY_SPECS", "1")))
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
    let mut value: Value = serde_json::from_str(text.trim())
        .context("The declaration reader did not print declaration JSON")?;
    let (mut agents, mut team, mut users, mut groups) = (Vec::new(), None, Vec::new(), Vec::new());
    let (mut ontologies, mut code) = Default::default();
    if specs && let Some(found) = value.get_mut("declarations").map(Value::take) {
        let list = |key: &str| value[key].as_array().cloned().unwrap_or_default();
        agents = list("agents");
        users = list("users");
        groups = list("groups");
        team = value["team"].as_str().map(str::to_owned);
        if let Some(found) = value.get("ontologies").filter(|o| !o.is_null()) {
            ontologies = serde_json::from_value(found.clone())
                .context("The declaration reader printed ontologies that are not a project's")?;
        }
        if let Some(found) = value.get("code").filter(|c| !c.is_null()) {
            code = serde_json::from_value(found.clone())
                .context("The declaration reader printed agents' code that is not paths")?;
        }
        value = found;
    }
    Ok(Declarations {
        digest: digest(&value),
        value,
        agents,
        team,
        users,
        groups,
        ontologies,
        code,
        releases: Default::default(),
        sources: Default::default(),
    })
}

/// Read each deploying agent's own code (`releases`): its `code` (an entry module, or a folder
/// with its own package.json or pyproject.toml), else `agents/<key>/` when the project has that
/// folder, from the project's directory. When the entry is code rather than the project's
/// `tilde.config.ts` (`tilde deploy` from one agent's folder), the one deploying agent with
/// neither releases the entry's own declarations; when it is the folder's own tilde.config.ts
/// declaring only that agent, the folder's package code is its release. Any other agent with
/// neither has its code in another repository: its spec is applied, and its release is that
/// repository's to register.
pub async fn releases(
    project: &Project,
    entry: &str,
    declarations: &mut Declarations,
) -> Result<()> {
    let deploying: Vec<String> = declarations
        .agents
        .iter()
        .filter(|agent| agent.get("deployment").is_some())
        .filter_map(|agent| agent["key"].as_str().map(str::to_owned))
        .collect();
    let mut elsewhere = Vec::new();
    for key in &deploying {
        let (path, explicit) = match declarations.code.get(key) {
            Some(code) => (project.dir.join(code), true),
            None => (project.dir.join("agents").join(key), false),
        };
        if !path.exists() {
            if explicit {
                bail!("Agent {key}: its code, {}, does not exist", path.display());
            }
            elsewhere.push(key.clone());
            continue;
        }
        let (dir, entry) = match path.is_dir() {
            true => (path.clone(), None),
            false => (
                path.parent()
                    .context("an entry module has a folder")?
                    .to_path_buf(),
                Some(path.clone()),
            ),
        };
        let code = Project::find(&dir)?;
        if path.is_dir() && code.dir != path.canonicalize()? {
            bail!(
                "Agent {key}: {} has no package.json or pyproject.toml of its own",
                path.display()
            );
        }
        let entry = match entry {
            Some(file) => file.to_string_lossy().into_owned(),
            None => code.code_entry()?,
        };
        eprintln!("Agent {key}: reading its code in {}", code.dir.display());
        let read = self::declarations(&code, &entry, false, false).await?;
        declarations.releases.insert(key.clone(), read.value);
        declarations
            .sources
            .insert(key.clone(), (code.dir.clone(), entry));
    }
    let entry_is_code = std::path::Path::new(entry).file_name() != Some("tilde.config.ts".as_ref());
    if entry_is_code {
        let declares = ["prompts", "skills", "tools"].iter().any(|kind| {
            declarations.value[kind]
                .as_array()
                .is_some_and(|items| !items.is_empty())
        });
        match &elsewhere[..] {
            [] if declares => bail!(
                "This entry declares prompts, skills or tools, but no exported agent declares \
                 deploy without code of its own: add `deploy: gateway()` (or `lambda(arn)`, \
                 `sidecar()`) to the agent whose code this is"
            ),
            [] => {}
            [key] => {
                declarations
                    .releases
                    .insert(key.clone(), declarations.value.clone());
                declarations
                    .sources
                    .insert(key.clone(), (project.dir.clone(), entry.to_owned()));
                elsewhere.clear();
            }
            keys => bail!(
                "Agents {} declare deploy but have no code of their own, and this entry can be the \
                 code of only one: put each in agents/<key>/, or name it with defineAgent({{ code }})",
                keys.join(", ")
            ),
        }
    }
    // `tilde deploy` from an agent's own folder (named after its key, or under `agents/`): its
    // tilde.config.ts declares only that agent, and the folder's package is its code. A
    // config-only repo declaring one remote agent is not its folder, whatever package it has.
    let own_folder = |key: &str| {
        project.dir.file_name().is_some_and(|name| name == key)
            || project
                .dir
                .parent()
                .and_then(|parent| parent.file_name())
                .is_some_and(|name| name == "agents")
    };
    if let ([key], 1) = (&elsewhere[..], declarations.agents.len())
        && own_folder(key)
        && let Ok(code_entry) = project.code_entry()
        && project.dir.join(&code_entry).is_file()
    {
        eprintln!("Agent {key}: reading its code in {}", project.dir.display());
        let read = self::declarations(project, &code_entry, false, false).await?;
        declarations.releases.insert(key.clone(), read.value);
        elsewhere.clear();
    }
    for key in elsewhere {
        eprintln!(
            "Agent {key}: no code here (no agents/{key}/, no code), so its spec is applied and its \
             release is registered from its own repository"
        );
    }
    Ok(())
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
