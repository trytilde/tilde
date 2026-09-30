//! Local sample skills, so every Skills screen has something to show: built-in and provider
//! catalog groups, a Git repository, editor groups with a few authored skills, and some of them
//! given to the example agents. Seeding skips what already exists; the GitHub-backed groups are
//! seeded only with `ENGINE_GITHUB_TOKEN` set, and a failed sync is logged rather than stopping
//! the examples. Absent from release builds.
use crate::database::Pool;
use secrecy::SecretString;
use tilde::{
    agent::avatar::ObjectStore,
    skills::{Skills, Upload, github::GitHub},
};
use uuid::Uuid;

const MASTRA: &str = "c1d2a9e4-6b38-4f75-a1c0-8f4e6d2b9c04";
const CREWAI: &str = "4a1d7c90-8e53-4b2f-9c16-d0e7f3a5b609";
const AGNO: &str = "e3f6c8a2-1b47-4d95-a6c3-9f0e2d8b5a08";

fn text(path: &str, content: &str) -> Upload {
    Upload::Data {
        path: path.into(),
        data: content.as_bytes().to_vec(),
        executable: false,
    }
}
fn skill_md(name: &str, description: &str, body: &str) -> String {
    format!("---\nname: {name}\ndescription: {description}\n---\n{body}")
}
fn agent(id: &str) -> Uuid {
    Uuid::parse_str(id).expect("example agent ids are UUIDs")
}

/// Seed after the examples are registered, since some groups are given to them.
pub async fn seed(pool: Pool, store: Option<ObjectStore>) {
    if let Err(error) = run(&pool, store).await {
        eprintln!("warning: sample skills were not seeded: {error}");
    }
}

async fn run(pool: &Pool, store: Option<ObjectStore>) -> Result<(), Box<dyn std::error::Error>> {
    let env = |key: &str, default: &str| std::env::var(key).unwrap_or_else(|_| default.into());
    let token = std::env::var("ENGINE_GITHUB_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
        .map(SecretString::from);
    let skills = Skills::new(pool.clone())
        .with_store(store)
        .with_github(GitHub::new(
            &env("ENGINE_GITHUB_API_URL", "https://api.github.com"),
            &env("ENGINE_GITHUB_RAW_URL", "https://raw.githubusercontent.com"),
            token,
        )?);
    let existing = skills.sources().await?;
    let find = |name: &str| existing.iter().find(|s| s.name == name).map(|s| s.id);

    let (runtime, _) = skills.enable_catalog("tilde-runtime").await?;
    skills.enable_catalog("tilde-working-style").await?;
    let github = std::env::var("ENGINE_GITHUB_TOKEN").is_ok_and(|t| !t.is_empty());
    if github && let Err(error) = skills.enable_catalog("vercel").await {
        eprintln!("warning: the Vercel catalog group did not sync: {error}");
    }
    if github && find("Anthropic skills").is_none() {
        // A failing first sync is recorded on the group, which the Skills page then flags.
        skills
            .add_git(
                "Anthropic skills",
                "https://github.com/anthropics/skills",
                "main",
                "skills",
            )
            .await?;
    }

    let support = match find("Support playbooks") {
        Some(id) => id,
        None => {
            let id = skills.create_editor("Support playbooks").await?;
            skills
                .write(
                    id,
                    "refund-policy",
                    vec![
                        text(
                            "SKILL.md",
                            &skill_md(
                                "refund-policy",
                                "Decide and explain refunds. Use when a customer asks for their money back or disputes a charge.",
                                "# Refund policy\n\nRead `reference/policy.md` before answering.\n\n1. Confirm the order and its date.\n2. Refund in full within 30 days; offer store credit after that.\n3. Explain the outcome in one short paragraph.\n",
                            ),
                        ),
                        text(
                            "reference/policy.md",
                            "# Policy\n\n- Full refunds within **30 days** of delivery.\n- Store credit up to 90 days.\n- Digital goods are refunded only when unused.\n",
                        ),
                    ],
                    "Seeded",
                    true,
                )
                .await?;
            skills
                .write(
                    id,
                    "escalation-handoff",
                    vec![text(
                        "SKILL.md",
                        &skill_md(
                            "escalation-handoff",
                            "Hand a conversation to a person. Use when the customer is upset, asks for a human, or the request is outside policy.",
                            "# Escalation handoff\n\n- Summarise the issue in two sentences.\n- Tell the customer a person will follow up, and when.\n- Never promise an outcome.\n",
                        ),
                    )],
                    "Seeded",
                    true,
                )
                .await?;
            id
        }
    };
    if find("Drafts").is_none() {
        let id = skills.create_editor("Drafts").await?;
        skills
            .write(
                id,
                "tone-of-voice",
                vec![text(
                    "SKILL.md",
                    &skill_md(
                        "tone-of-voice",
                        "Match the brand's tone. Use when writing anything a customer will read.",
                        "# Tone of voice\n\nWarm, plain and brief. Prefer *you* over *the customer*.\n",
                    ),
                )],
                "Seeded",
                true,
            )
            .await?;
    }

    // Given to examples: a whole group on Mastra, one skill of an otherwise-off group on CrewAI,
    // and a whole group on Agno. Only on the first seed, so later changes in the UI stick.
    if find("Support playbooks").is_none() {
        let refund = skills
            .skills(Some(support))
            .await?
            .into_iter()
            .find(|s| s.row.name == "refund-policy")
            .map(|s| s.row.id);
        skills.assign_source(agent(MASTRA), runtime).await?;
        skills.enable_source(agent(MASTRA), runtime, true).await?;
        skills.assign_source(agent(CREWAI), support).await?;
        if let Some(refund) = refund {
            skills.assign_skill(agent(CREWAI), refund).await?;
        }
        skills.assign_source(agent(AGNO), support).await?;
        skills.enable_source(agent(AGNO), support, true).await?;
    }
    Ok(())
}
