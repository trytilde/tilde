//! Reading an agent project: which language runtime hosts it, what to call it in the registry,
//! which module declares its prompts and skills, and how to start it.
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Runtime {
    Node,
    Python,
}

impl Runtime {
    pub fn label(self) -> &'static str {
        match self {
            Runtime::Node => "node",
            Runtime::Python => "python",
        }
    }
}

/// `tilde.toml`, committed with the project. Every field is optional: a project that follows
/// its ecosystem's conventions needs no file at all.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Manifest {
    pub name: Option<String>,
    pub entry: Option<String>,
    pub run: Option<String>,
}

pub struct Project {
    pub dir: PathBuf,
    pub runtime: Runtime,
    pub name: String,
    pub manifest: Manifest,
}

impl Project {
    /// Walks up from `start` to the nearest directory holding a recognised project manifest.
    pub fn find(start: &Path) -> Result<Self> {
        let start = start
            .canonicalize()
            .with_context(|| format!("{} does not exist", start.display()))?;
        for dir in start.ancestors() {
            let runtime = if dir.join("package.json").is_file() {
                Runtime::Node
            } else if dir.join("pyproject.toml").is_file() {
                Runtime::Python
            } else {
                continue;
            };
            let manifest = match fs::read_to_string(dir.join("tilde.toml")) {
                Ok(text) => toml::from_str(&text).context("tilde.toml could not be read")?,
                Err(_) => Manifest::default(),
            };
            let name = match &manifest.name {
                Some(name) => name.clone(),
                None => package_name(dir, runtime)?,
            };
            return Ok(Self {
                dir: dir.to_path_buf(),
                runtime,
                name,
                manifest,
            });
        }
        bail!(
            "No agent project found in {} or its parents (expected a package.json or pyproject.toml)",
            start.display()
        )
    }

    /// The module `tilde deploy` reads declarations from, relative to the project.
    pub fn entry(&self) -> Result<String> {
        if let Some(entry) = &self.manifest.entry {
            return Ok(entry.clone());
        }
        match self.runtime {
            // Mirrors the TypeScript CLI's own default order.
            Runtime::Node => {
                if let Some(main) = manifest_json(&self.dir)?
                    .get("main")
                    .and_then(Value::as_str)
                {
                    return Ok(main.to_string());
                }
                for candidate in ["dist/index.js", "src/index.ts", "index.js"] {
                    if self.dir.join(candidate).is_file() {
                        return Ok(candidate.to_string());
                    }
                }
                bail!(
                    "Could not find this project's entry module. Build it first, or set entry in tilde.toml"
                )
            }
            Runtime::Python => {
                for candidate in ["src/agent.py", "agent.py", "src/main.py", "main.py"] {
                    if self.dir.join(candidate).is_file() {
                        return Ok(candidate.to_string());
                    }
                }
                bail!("Could not find this project's entry module. Set entry in tilde.toml")
            }
        }
    }

    /// The command `tilde dev` runs to start the agent, as program plus arguments.
    pub fn run_command(&self) -> Result<Vec<String>> {
        if let Some(run) = &self.manifest.run {
            let parts: Vec<String> = run.split_whitespace().map(str::to_string).collect();
            if parts.is_empty() {
                bail!("run in tilde.toml is empty");
            }
            return Ok(parts);
        }
        match self.runtime {
            Runtime::Node => {
                let scripts = manifest_json(&self.dir)?;
                let scripts = scripts.get("scripts").and_then(Value::as_object);
                for script in ["dev", "start"] {
                    if scripts.is_some_and(|scripts| scripts.contains_key(script)) {
                        return Ok(vec!["npm".into(), "run".into(), script.into()]);
                    }
                }
                Ok(vec!["node".into(), self.entry()?])
            }
            Runtime::Python => {
                // The host that dials the gateway is usually a different module from the one
                // declaring prompts, and it must run in the project's own environment.
                let script = ["main.py", "src/main.py"]
                    .into_iter()
                    .find(|candidate| self.dir.join(candidate).is_file())
                    .map(str::to_string)
                    .unwrap_or(self.entry()?);
                Ok(vec![self.python(), script])
            }
        }
    }
}

impl Project {
    /// The interpreter for this project: an activated environment, then the nearest `.venv`
    /// (uv keeps one at the root of a workspace), then whatever `python` is on PATH.
    fn python(&self) -> String {
        let interpreter = |dir: &Path| {
            ["bin/python", "Scripts/python.exe"]
                .iter()
                .map(|relative| dir.join(relative))
                .find(|candidate| candidate.is_file())
        };
        if let Some(active) = std::env::var_os("VIRTUAL_ENV")
            && let Some(found) = interpreter(Path::new(&active))
        {
            return found.to_string_lossy().into_owned();
        }
        for dir in self.dir.ancestors() {
            if let Some(found) = interpreter(&dir.join(".venv")) {
                return found.to_string_lossy().into_owned();
            }
        }
        "python".into()
    }
}

fn manifest_json(dir: &Path) -> Result<Value> {
    let text =
        fs::read_to_string(dir.join("package.json")).context("package.json is unreadable")?;
    serde_json::from_str(&text).context("package.json is not valid JSON")
}

/// The project's own name, unscoped, as the default agent name.
fn package_name(dir: &Path, runtime: Runtime) -> Result<String> {
    let named = match runtime {
        Runtime::Node => manifest_json(dir)?
            .get("name")
            .and_then(Value::as_str)
            .map(|name| name.rsplit('/').next().unwrap_or(name).to_string()),
        Runtime::Python => toml::from_str::<Value>(
            &fs::read_to_string(dir.join("pyproject.toml"))
                .context("pyproject.toml is unreadable")?,
        )
        .context("pyproject.toml could not be read")?
        .get("project")
        .and_then(|project| project.get("name"))
        .and_then(Value::as_str)
        .map(str::to_string),
    };
    named
        .or_else(|| {
            dir.file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })
        .context("Could not name this project; set name in tilde.toml")
}
