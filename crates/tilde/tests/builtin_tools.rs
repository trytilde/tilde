//! Built-in tools follow the capabilities an agent already holds: messaging another agent
//! crosses IAM, thread membership and the run queue; conversation tools read the thread store.
mod common;
use common::invocation::{Fixture, create_agent};
use serde_json::json;
use tilde::chat as application;
use tilde::proto::tilde::types::v1 as types;

fn all() -> types::TargetPermission {
    types::TargetPermission {
        mode: types::TargetSelection::All.into(),
        ..Default::default()
    }
}

#[tokio::test]
async fn agent_and_thread_tools_follow_the_agents_capabilities() {
    // No capability beyond tool invocation: none of the capability-gated tools exist.
    let bare = Fixture::new(41, Default::default(), common::invocation::all_tools()).await;
    assert!(bare.tools().await.iter().all(|t| {
        let name = t["name"].as_str().unwrap();
        !name.starts_with("agents.") && !name.starts_with("thread.")
    }));
    assert!(
        bare.invoke("agents.list", json!({}))
            .await
            .get("error")
            .is_some()
    );
    bare.close().await;

    // The specialist must exist before the operator's grant can name it, so build the
    // operator's fixture with a placeholder grant and then narrow it to the real agent.
    let fx = Fixture::new(
        42,
        Default::default(),
        types::Capabilities {
            tools_invoke: all().into(),
            agents_read: all().into(),
            agents_invoke: all().into(),
            thread_read: types::BinaryPermission::Yes.into(),
            ..Default::default()
        },
    )
    .await;
    let specialist = create_agent(&fx.agents, "Specialist", Default::default()).await;
    let names: Vec<String> = fx
        .tools()
        .await
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_owned())
        .collect();
    for expected in [
        "agents.list",
        "agents.message",
        "agents.wait",
        "thread.participants",
        "thread.search",
    ] {
        assert!(
            names.iter().any(|n| n == expected),
            "{expected} missing from {names:?}"
        );
    }
    let listed = fx.output("agents.list", json!({})).await;
    assert_eq!(
        listed["agents"],
        json!([{"agent_id": specialist, "name": "Specialist"}]),
        "never lists the caller"
    );

    assert!(
        fx.invoke(
            "agents.message",
            json!({"agent_id": fx.agent, "message":"hi"})
        )
        .await
        .get("error")
        .is_some()
    );
    assert!(
        fx.invoke("agents.message", json!({"agent_id": specialist}))
            .await
            .get("error")
            .is_some(),
        "schema is enforced"
    );
    let started = fx
        .output(
            "agents.message",
            json!({"agent_id": specialist, "message":"Summarise the incident"}),
        )
        .await;
    let roster = fx.output("thread.participants", json!({})).await;
    assert!(
        roster["participants"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["agent_id"] == json!(specialist)),
        "messaging an agent brings it into the conversation: {roster}"
    );
    let pg = fx.db.pool.get().await.unwrap();
    let run = uuid::Uuid::parse_str(started["run_id"].as_str().unwrap()).unwrap();
    let objective: String = pg
        .query_one(
            "SELECT objective FROM chat_runs WHERE id=$1 AND agent_id=$2",
            &[&run, &specialist],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(objective, "Summarise the incident");
    let waiting = fx
        .output("agents.wait", json!({"run_id": run, "timeout_seconds": 1}))
        .await;
    assert_eq!(
        (waiting["finished"].clone(), waiting["messages"].clone()),
        (json!(false), json!([]))
    );

    let user_participant = roster["participants"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| !p["user_id"].is_null())
        .unwrap()["participant_id"]
        .as_str()
        .unwrap()
        .to_owned();
    fx.chat
        .post(application::PostMessage {
            id: uuid::Uuid::new_v4().to_string(),
            thread_id: fx.thread.to_string(),
            participant_id: user_participant,
            text: "The rollback finished at noon".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let found = fx
        .output("thread.search", json!({"query":"rollback"}))
        .await;
    assert_eq!(
        found["messages"][0]["text"],
        "The rollback finished at noon"
    );
    drop(pg);
    fx.close().await;
}
