//! Wikis through the agent runtime: an invocation's wiki tools write pages into linked wikis with
//! revisions that search and grep find, stale writes are refused, read links offer reading only,
//! and an agent's own wiki goes with the agent.
mod common;
use common::invocation::{Fixture, all_tools};
use serde_json::{Value, json};
use tilde::wikis::Wikis;
use uuid::Uuid;

fn wikis(fx: &Fixture) -> Wikis {
    fx.chat.tools.as_ref().unwrap().wikis.clone()
}
async fn wiki(wikis: &Wikis, name: &str, agent: Option<Uuid>) -> Uuid {
    wikis.create_wiki(name, "", agent).await.unwrap().id
}
fn names(tools: &[Value]) -> Vec<String> {
    tools
        .iter()
        .filter_map(|t| t["name"].as_str())
        .filter(|n| n.starts_with("wiki."))
        .map(str::to_owned)
        .collect()
}
fn write(agent: Option<Uuid>) -> tilde::wikis::Write {
    tilde::wikis::Write {
        call: Uuid::new_v4(),
        agent,
        thread: None,
    }
}
fn code(result: &Value) -> &str {
    result["error"]["code"].as_str().unwrap_or_default()
}

#[tokio::test]
async fn an_agent_writes_pages_that_search_and_grep_find_and_stale_writes_are_refused() {
    let fx = Fixture::new(101, Default::default(), all_tools()).await;
    let wikis = wikis(&fx);
    assert!(names(&fx.tools().await).is_empty());
    let own = wiki(&wikis, "operator", Some(fx.agent)).await;
    assert_eq!(
        names(&fx.tools().await),
        [
            "wiki.search",
            "wiki.grep",
            "wiki.read",
            "wiki.list",
            "wiki.write",
            "wiki.delete"
        ]
    );

    let created = fx
        .output(
            "wiki.write",
            json!({"path":"/runbooks/deploys/","title":"Deploying the website","expected_revision":0,
                "content":"# Deploying\n\nRun `task deploy` from main.\nRollbacks use `task rollback`."}),
        )
        .await;
    assert_eq!(
        (created["path"].clone(), created["revision"].clone()),
        (json!("runbooks/deploys"), json!(1))
    );
    // Creating again at the path is refused: the agent reads and updates instead.
    let again = fx
        .invoke(
            "wiki.write",
            json!({"path":"runbooks/deploys","title":"x","content":"y","expected_revision":0}),
        )
        .await;
    assert_eq!(code(&again), "already_exists", "{again}");

    let found = fx
        .output("wiki.search", json!({"query":"how do I deploy?"}))
        .await;
    assert_eq!(found["pages"][0]["path"], json!("runbooks/deploys"));
    let lines = fx
        .output("wiki.grep", json!({"pattern":"task rollback"}))
        .await;
    assert_eq!(
        (
            lines["lines"][0]["line"].clone(),
            lines["lines"][0]["text"].clone()
        ),
        (json!(4), json!("Rollbacks use `task rollback`."))
    );
    // A literal's special characters match themselves; a bad regular expression is the agent's to fix.
    let literal = fx
        .output("wiki.grep", json!({"pattern":"`task deploy`"}))
        .await;
    assert_eq!(literal["lines"].as_array().unwrap().len(), 1);
    let bad = fx
        .invoke("wiki.grep", json!({"pattern":"(unclosed","regex":true}))
        .await;
    assert_eq!(code(&bad), "invalid_argument", "{bad}");

    let read = fx
        .output("wiki.read", json!({"path":"runbooks/deploys"}))
        .await;
    assert_eq!(
        (
            read["revision"].clone(),
            read["from"].clone(),
            read["to"].clone(),
            read["total"].clone()
        ),
        (json!(1), json!(1), json!(4), json!(4))
    );
    let past = fx
        .invoke(
            "wiki.read",
            json!({"path":"runbooks/deploys","from_line":9}),
        )
        .await;
    assert_eq!(code(&past), "invalid_argument", "{past}");
    let updated = fx
        .output(
            "wiki.write",
            json!({"path":"runbooks/deploys","expected_revision":1,
                "content":"# Deploying\n\nRun `task deploy` from main after CI passes."}),
        )
        .await;
    assert_eq!(updated["revision"], json!(2));
    // A write based on revision 1 now would overwrite revision 2: it is refused.
    let stale = fx
        .invoke(
            "wiki.write",
            json!({"path":"runbooks/deploys","expected_revision":1,"content":"Something else"}),
        )
        .await;
    assert_eq!(code(&stale), "aborted", "{stale}");

    let page = wikis
        .page_at(own, "runbooks/deploys")
        .await
        .unwrap()
        .unwrap();
    let history = wikis.revisions(page.id).await.unwrap();
    assert_eq!(
        history
            .iter()
            .map(|r| (r.revision, r.operation.as_str(), r.agent_id, r.thread_id))
            .collect::<Vec<_>>(),
        [
            (2, "update", Some(fx.agent), Some(fx.thread)),
            (1, "create", Some(fx.agent), Some(fx.thread))
        ]
    );
    assert_eq!(page.title, "Deploying the website");

    // Small changes go as edits; each must match the page exactly once.
    let missing = fx
        .invoke(
            "wiki.write",
            json!({"path":"runbooks/deploys","expected_revision":2,
                "edits":[{"old":"after CI passes","new":"after CI and review pass"},{"old":"nowhere","new":"x"}]}),
        )
        .await;
    assert_eq!(code(&missing), "invalid_argument", "{missing}");
    assert!(
        missing["error"]["message"]
            .as_str()
            .unwrap()
            .starts_with("Edit 2:"),
        "{missing}"
    );
    let edited = fx
        .output(
            "wiki.write",
            json!({"path":"runbooks/deploys","expected_revision":2,
                "edits":[{"old":"after CI passes","new":"after CI and review pass"}]}),
        )
        .await;
    assert_eq!(edited["revision"], json!(3));
    assert_eq!(
        wikis
            .page_at(own, "runbooks/deploys")
            .await
            .unwrap()
            .unwrap()
            .content,
        "# Deploying\n\nRun `task deploy` from main after CI and review pass."
    );

    // Deleting keeps the history.
    fx.output(
        "wiki.delete",
        json!({"path":"runbooks/deploys","expected_revision":3}),
    )
    .await;
    assert!(
        wikis
            .page_at(own, "runbooks/deploys")
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        wikis.revision(page.id, 4).await.unwrap().operation,
        "delete"
    );
    fx.close().await;
}

#[tokio::test]
async fn a_read_link_offers_reading_only_and_another_agents_wiki_stays_its_own() {
    let fx = Fixture::new(102, Default::default(), all_tools()).await;
    let wikis = wikis(&fx);
    let handbook = wiki(&wikis, "handbook", None).await;
    wikis
        .create_page(
            &write(None),
            handbook,
            "expenses",
            "Expenses",
            "Every expense needs a receipt.",
        )
        .await
        .unwrap();
    wikis.set_link(fx.agent, handbook, "read").await.unwrap();
    assert_eq!(
        names(&fx.tools().await),
        ["wiki.search", "wiki.grep", "wiki.read", "wiki.list"]
    );
    let listed = fx.output("wiki.list", json!({})).await;
    assert_eq!(listed["pages"][0]["path"], json!("expenses"));
    let refused = fx
        .invoke(
            "wiki.write",
            json!({"path":"expenses","content":"No receipts","expected_revision":1}),
        )
        .await;
    assert!(refused.get("error").is_some(), "{refused}");

    let other = common::invocation::create_agent(&fx.agents, "Other", Default::default()).await;
    let theirs = wiki(&wikis, "other", Some(other)).await;
    assert!(wikis.set_link(fx.agent, theirs, "read").await.is_err());
    assert!(wikis.remove_link(other, theirs).await.is_err());
    fx.close().await;
}

#[tokio::test]
async fn deleting_an_agent_deletes_its_own_wiki_and_keeps_shared_ones() {
    let fx = Fixture::new(103, Default::default(), all_tools()).await;
    let wikis = wikis(&fx);
    let own = wiki(&wikis, "operator", Some(fx.agent)).await;
    // An agent has one wiki of its own, and one bank.
    let second = wikis
        .create_wiki("operator-2", "", Some(fx.agent))
        .await
        .unwrap_err();
    assert!(
        second.to_string().contains("already has its own wiki"),
        "{second}"
    );
    let memory = fx.chat.tools.as_ref().unwrap().memory.clone();
    let bank = |name: &'static str| {
        let memory = memory.clone();
        let agent = fx.agent;
        async move { memory.create_bank(name, "", "shared", Some(agent)).await }
    };
    bank("operator").await.unwrap();
    let again = bank("operator-2").await.unwrap_err();
    assert!(
        again.to_string().contains("already has its own bank"),
        "{again}"
    );
    let shared = wiki(&wikis, "handbook", None).await;
    wikis
        .set_link(fx.agent, shared, "read_write")
        .await
        .unwrap();
    fx.output(
        "wiki.write",
        json!({"wiki":"handbook","path":"onboarding","title":"Onboarding","content":"Day one.","expected_revision":0}),
    )
    .await;
    fx.agents.pause(fx.agent).await.unwrap();
    fx.agents.delete(fx.agent).await.unwrap();
    assert!(wikis.wiki(own).await.is_err());
    let kept = wikis.wiki(shared).await.unwrap();
    assert_eq!((kept.page_count, kept.agent_ids.len()), (1, 0));
    // A deleted agent gets no wikis or links.
    assert!(wikis.set_link(fx.agent, shared, "read").await.is_err());
    fx.close().await;
}

#[tokio::test]
async fn a_retried_write_returns_its_result_instead_of_writing_again() {
    let fx = Fixture::new(105, Default::default(), all_tools()).await;
    let wikis = wikis(&fx);
    let wiki = wiki(&wikis, "handbook", None).await;
    let agent = Some(fx.agent);
    let (create, update, delete) = (write(agent), write(agent), write(agent));
    let page = wikis
        .create_page(&create, wiki, "faq", "FAQ", "First answer.")
        .await
        .unwrap();
    let again = wikis
        .create_page(&create, wiki, "faq", "FAQ", "First answer.")
        .await
        .unwrap();
    assert_eq!(again.id, page.id);
    // The same call id for another write is the caller's mistake, never a silent success.
    let elsewhere = wikis
        .create_page(&create, wiki, "other", "Other", "Something else.")
        .await
        .unwrap_err();
    assert!(
        elsewhere.to_string().contains("different write"),
        "{elsewhere}"
    );
    let body = || Some(tilde::wikis::Body::Replace("Second answer.".into()));
    let updated = wikis
        .update_page(&update, page.id, 1, None, body())
        .await
        .unwrap();
    // Retried at the revision it read, after it landed: the page it made, not a stale error.
    let retried = wikis
        .update_page(&update, page.id, 1, None, body())
        .await
        .unwrap();
    assert_eq!((updated.revision, retried.revision), (2, 2));
    // The same call id for the next edit is not a retry of the first: refused, not dropped.
    let next = wikis
        .update_page(&update, page.id, 2, None, body())
        .await
        .unwrap_err();
    assert!(next.to_string().contains("different write"), "{next}");
    assert_eq!(wikis.revisions(page.id).await.unwrap().len(), 2);
    assert!(
        wikis
            .move_page(&update, page.id, 2, "help/faq")
            .await
            .unwrap_err()
            .to_string()
            .contains("different write")
    );
    // Another write at the old revision is stale, and names the current one.
    let stale = wikis
        .update_page(&write(agent), page.id, 1, None, body())
        .await
        .unwrap_err();
    assert!(stale.to_string().contains("revision 2"), "{stale}");

    // A retried delete succeeds and leaves a page since created at the path alone.
    wikis.delete_page(&delete, page.id, 2).await.unwrap();
    let recreated = wikis
        .create_page(&write(agent), wiki, "faq", "FAQ", "New answers.")
        .await
        .unwrap();
    // The retry repeats the request it retries: the same revision, 2.
    wikis.delete_at(&delete, wiki, "faq", 2).await.unwrap();
    assert_eq!(
        wikis.page_at(wiki, "faq").await.unwrap().unwrap().id,
        recreated.id
    );
    fx.close().await;
}
