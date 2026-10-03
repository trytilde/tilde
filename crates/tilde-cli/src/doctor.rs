//! `tilde doctor`: answer "why is this not working" without reading any source.
use crate::{api::Client, dev::check_gateway, discover, project::Project};
use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

#[derive(Args)]
pub struct Doctor {
    #[arg(long, env = "TILDE_URL", default_value = "http://127.0.0.1:8080")]
    pub url: String,
    #[arg(long, env = "TILDE_API_KEY", hide_env_values = true)]
    pub api_key: Option<String>,
    #[arg(long, default_value = ".", hide = true)]
    pub dir: PathBuf,
}

fn report(label: &str, outcome: Result<String>) -> bool {
    match outcome {
        Ok(detail) => {
            println!("  ok    {label}: {detail}");
            true
        }
        Err(error) => {
            // Indent any continuation lines so a multi-line cause stays readable.
            let detail = format!("{error}").replace('\n', "\n        ");
            println!("  FAIL  {label}: {detail}");
            false
        }
    }
}

pub async fn run(args: Doctor) -> Result<()> {
    println!("tilde {}", env!("CARGO_PKG_VERSION"));
    let mut healthy = true;

    let project = Project::find(&args.dir);
    let described = project
        .as_ref()
        .map(|project| {
            format!(
                "{} ({}) in {}",
                project.name,
                project.runtime.label(),
                project.dir.display()
            )
        })
        .map_err(|error| anyhow::anyhow!("{error}"));
    healthy &= report("project", described);

    if let Ok(project) = &project {
        let entry = project.entry();
        let found = entry
            .as_ref()
            .map(|entry| entry.clone())
            .map_err(|error| anyhow::anyhow!("{error}"));
        healthy &= report("entry", found);
        if let Ok(entry) = entry {
            let read = discover::declarations(project, &entry, true)
                .await
                .map(|declarations| {
                    let count = |group: &str| {
                        declarations
                            .value
                            .get(group)
                            .and_then(|value| value.as_array())
                            .map_or(0, Vec::len)
                    };
                    format!(
                        "{} prompt(s), {} skill(s), {} tool(s)",
                        count("prompts"),
                        count("skills"),
                        count("tools")
                    )
                });
            healthy &= report("declarations", read);
        }
        healthy &= report(
            "run command",
            project.run_command().map(|command| command.join(" ")),
        );
    }

    match Client::new(&args.url, args.api_key.clone()) {
        Ok(client) => {
            healthy &= report(
                "gateway",
                check_gateway(&client).await.map(|()| args.url.clone()),
            );
        }
        Err(error) => healthy &= report("gateway", Err(error)),
    }

    println!();
    if healthy {
        println!("Everything checks out. Run `tilde dev` to start your agent.");
        return Ok(());
    }
    anyhow::bail!("Some checks failed")
}
