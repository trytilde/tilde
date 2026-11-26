//! The built-in skill and wiki link tools through the agent runtime: offered only with the
//! capabilities their runtime and management calls need, authorized against the agent they act
//! on and the source or wiki they give, and effective in the skills and wikis services.
mod common;
use common::invocation::{Fixture, all_tools, create_agent};
use serde_json::{Value, json};
use tilde::proto::tilde::types::v1 as types;
use uuid::Uuid;

fn all() -> types::TargetPermission {
    types::TargetPermission {
        mode: types::TargetSelection::All.into(),
        ..Default::default()
    }
}
/// The skill and wiki link tools the agent is offered, sorted.
fn offered(tools: &[Value]) -> Vec<String> {
    let mut names: Vec<String> = tools
        .iter()
        .filter_map(|t| t["name"].as_str())
        .filter(|n| n.starts_with("skills.") || ["wiki.link", "wiki.unlink"].contains(n))
        .map(str::to_owned)
        .collect();
    names.sort();
    names
}
fn code(result: &Value) -> &str {
    result["error"]["code"].as_str().unwrap_or_default()
}
/// A wiki named `name`, the own wiki of `agent` when given.
async fn wiki(fx: &Fixture, name: &str, agent: Option<Uuid>) -> Uuid {
    let wikis = fx.chat.tools.as_ref().unwrap().wikis.clone();
    let created = wikis.create_wiki(name, "", agent).await;
    created.unwrap().id
}
/// An editor skill source named `name`.
async fn editor(fx: &Fixture, name: &str) -> Uuid {
    let skills = tilde::skills::Skills::new(fx.db.pool.clone());
    let created = skills.create_editor(name).await;
    created.unwrap()
}
fn skill(name: &str, body: &str) -> Value {
    json!([{"path": "SKILL.md", "content": format!("---\nname: {name}\ndescription: {body}\n---\n\n{body}\n")}])
}

#[tokio::test]
async fn skill_and_wiki_link_tools_follow_capabilities_and_change_other_agents() {
    // A wiki linked but no capability for these tools: only the wiki's own tools.
    let bare = Fixture::new(61, Default::default(), all_tools()).await;
    let wikis = bare.chat.tools.as_ref().unwrap().wikis.clone();
    let shared = wiki(&bare, "Runbooks", None).await;
    bare.within(wikis.set_link(bare.agent, shared, "read_write"))
        .await
        .unwrap();
    assert!(offered(&bare.tools().await).is_empty());
    for (tool, input) in [
        (
            "skills.write",
            json!({"source": "x", "name": "x", "files": skill("x", "x")}),
        ),
        (
            "wiki.link",
            json!({"agent_id": Uuid::new_v4(), "access": "read"}),
        ),
    ] {
        let refused = bare.invoke(tool, input).await;
        assert!(refused.get("error").is_some(), "{tool}: {refused}");
    }
    bare.close().await;

    let mut fx = Fixture::new(
        62,
        Default::default(),
        types::Capabilities {
            tools_invoke: all().into(),
            skills_edit: all().into(),
            agents_edit_skills: all().into(),
            agents_edit_wikis: all().into(),
            ..Default::default()
        },
    )
    .await;
    let skills = tilde::skills::Skills::new(fx.db.pool.clone());
    let wikis = fx.chat.tools.as_ref().unwrap().wikis.clone();
    let specialist = fx
        .within(create_agent(&fx.agents, "Specialist", Default::default()))
        .await;
    let playbooks = editor(&fx, "Playbooks").await;
    for (name, agent, access) in [
        ("Runbooks", None, "read"),
        ("Notes", None, "read_write"),
        // Its own wiki is never another agent's.
        ("Operator", Some(fx.agent), "read_write"),
    ] {
        let id = wiki(&fx, name, agent).await;
        fx.within(wikis.set_link(fx.agent, id, access))
            .await
            .unwrap();
    }
    let tools = fx.tools().await;
    assert_eq!(
        offered(&tools),
        [
            "skills.assign",
            "skills.unassign",
            "skills.write",
            "wiki.link",
            "wiki.unlink"
        ]
    );
    let link = tools.iter().find(|t| t["name"] == "wiki.link").unwrap();
    let schema: Value = serde_json::from_str(link["inputSchemaJson"].as_str().unwrap()).unwrap();
    assert_eq!(
        schema["properties"]["wiki"]["enum"],
        json!(["Notes", "Runbooks"])
    );

    // Writing: created once, the same files again change nothing, only into a source it edits.
    let written = fx
        .output(
            "skills.write",
            json!({"source": "playbooks", "name": "triage", "files": skill("triage", "Triage an incident")}),
        )
        .await;
    assert_eq!(
        (written["skill"].clone(), written["created"].clone()),
        (json!("playbooks/triage"), json!(true))
    );
    let again = fx
        .output(
            "skills.write",
            json!({"source": "playbooks", "name": "triage", "files": skill("triage", "Triage an incident")}),
        )
        .await;
    assert_eq!(
        (again["created"].clone(), again["version"].clone()),
        (json!(false), written["version"].clone())
    );
    let missing = fx
        .invoke(
            "skills.write",
            json!({"source": "nowhere", "name": "triage", "files": skill("triage", "x")}),
        )
        .await;
    assert_eq!(code(&missing), "not_found", "{missing}");

    // Assigning: the whole source, then taking it away drops its skills.
    let given = || fx.within(skills.for_agent(specialist, None));
    for _ in 0..2 {
        fx.output(
            "skills.assign",
            json!({"agent_id": specialist, "source": "playbooks"}),
        )
        .await;
    }
    let names: Vec<String> = given().await.unwrap().into_iter().map(|s| s.name).collect();
    assert_eq!(names, ["triage"]);
    let neither = fx
        .invoke("skills.assign", json!({"agent_id": specialist}))
        .await;
    assert_eq!(code(&neither), "invalid_argument", "{neither}");
    fx.output(
        "skills.unassign",
        json!({"agent_id": specialist, "source": "playbooks"}),
    )
    .await;
    assert!(given().await.unwrap().is_empty());
    fx.output(
        "skills.assign",
        json!({"agent_id": specialist, "skill": "playbooks/triage"}),
    )
    .await;
    assert_eq!(given().await.unwrap().len(), 1);
    fx.output(
        "skills.unassign",
        json!({"agent_id": specialist, "skill": "playbooks/triage"}),
    )
    .await;
    assert!(given().await.unwrap().is_empty());

    // Linking: up to the caller's own access, never to itself, never its own wiki.
    let linked = || fx.within(wikis.agent_wikis(specialist));
    let wider = fx
        .invoke(
            "wiki.link",
            json!({"agent_id": specialist, "wiki": "Runbooks", "access": "read_write"}),
        )
        .await;
    assert_eq!(code(&wider), "permission_denied", "{wider}");
    for (wiki, access) in [
        ("Runbooks", "read"),
        ("Notes", "read_write"),
        ("Notes", "read_write"),
    ] {
        let link = fx
            .output(
                "wiki.link",
                json!({"agent_id": specialist, "wiki": wiki, "access": access}),
            )
            .await;
        assert_eq!(link["access"], access);
    }
    let held: Vec<(String, String)> = linked()
        .await
        .unwrap()
        .into_iter()
        .map(|l| (l.wiki_name, l.access))
        .collect();
    assert_eq!(
        held,
        [
            ("Notes".to_owned(), "read_write".to_owned()),
            ("Runbooks".to_owned(), "read".to_owned())
        ]
    );
    for refused in [
        json!({"agent_id": fx.agent, "wiki": "Notes", "access": "read"}),
        json!({"agent_id": specialist, "wiki": "Operator", "access": "read"}),
    ] {
        let result = fx.invoke("wiki.link", refused).await;
        assert_eq!(code(&result), "invalid_argument", "{result}");
    }
    for _ in 0..2 {
        fx.output(
            "wiki.unlink",
            json!({"agent_id": specialist, "wiki": "Notes"}),
        )
        .await;
    }
    let held: Vec<String> = linked()
        .await
        .unwrap()
        .into_iter()
        .map(|l| l.wiki_name)
        .collect();
    assert_eq!(held, ["Runbooks"]);

    // Narrowed to the specialist and to reading the one source: no writing, and nothing for
    // another agent or from another source.
    let outsider = fx
        .within(create_agent(&fx.agents, "Outsider", Default::default()))
        .await;
    editor(&fx, "Secret").await;
    let narrowed = json!({
        "tools.invoke": {"mode": "all"},
        "agents.edit_skills": {"mode": "selected", "ids": [specialist]},
        "agents.edit_wikis": {"mode": "selected", "ids": [specialist]},
        "skills.read": {"mode": "selected", "ids": [playbooks]},
    });
    let update = format!(
        "UPDATE agents SET capabilities = '{narrowed}' WHERE id = '{}'",
        fx.agent
    );
    let pool = fx.db.pool.clone();
    fx.within(async move {
        pool.get()
            .await
            .unwrap()
            .execute(&update, &[])
            .await
            .unwrap()
    })
    .await;
    fx.next_invocation().await;
    assert_eq!(
        offered(&fx.tools().await),
        [
            "skills.assign",
            "skills.unassign",
            "wiki.link",
            "wiki.unlink"
        ]
    );
    for (tool, input) in [
        (
            "skills.assign",
            json!({"agent_id": outsider, "source": "playbooks"}),
        ),
        (
            "skills.assign",
            json!({"agent_id": specialist, "source": "secret"}),
        ),
        (
            "skills.unassign",
            json!({"agent_id": outsider, "source": "playbooks"}),
        ),
        (
            "wiki.link",
            json!({"agent_id": outsider, "wiki": "Runbooks", "access": "read"}),
        ),
        (
            "wiki.unlink",
            json!({"agent_id": outsider, "wiki": "Runbooks"}),
        ),
    ] {
        let refused = fx.invoke(tool, input.clone()).await;
        assert_eq!(
            code(&refused),
            "permission_denied",
            "{tool} {input}: {refused}"
        );
    }
    fx.output(
        "skills.assign",
        json!({"agent_id": specialist, "source": "playbooks"}),
    )
    .await;
    let given = fx.within(skills.for_agent(specialist, None)).await.unwrap();
    assert_eq!(given.len(), 1);
    fx.close().await;
}
