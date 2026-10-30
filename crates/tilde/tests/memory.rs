//! Memory through the agent runtime: an invocation's memory tools write into linked banks, a
//! person's facts stay out of agents' reach, per-person banks keep people apart, and the
//! embedding worker, through an inference connection, lets recall find memories by meaning.
mod common;
use common::invocation::{Fixture, all_tools};
use secrecy::SecretString;
use serde_json::{Value, json};
use tilde::connections::service::Connections;
use tilde::memory::{Draft, Filter, Memory, Person, Source};
use uuid::Uuid;

fn memory(fx: &Fixture) -> Memory {
    fx.chat.tools.as_ref().unwrap().memory.clone()
}
async fn bank(memory: &Memory, name: &str, scope: &str, agent: Option<Uuid>) -> Uuid {
    memory.create_bank(name, "", scope, agent).await.unwrap().id
}
fn names(tools: &[Value]) -> Vec<String> {
    tools
        .iter()
        .filter_map(|t| t["name"].as_str())
        .filter(|n| n.starts_with("memory."))
        .map(str::to_owned)
        .collect()
}
fn titles(output: &Value) -> Vec<&str> {
    output["memories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["title"].as_str().unwrap())
        .collect()
}
async fn person_fact(memory: &Memory, bank: Uuid, title: &str, identity: Option<Uuid>) -> Uuid {
    memory
        .create(
            Uuid::new_v4(),
            bank,
            Draft {
                kind: "preferences".into(),
                path: "team".into(),
                title: title.into(),
                content: format!("{title}, as a person wrote it."),
                valid_from: None,
                valid_to: None,
                ongoing: false,
                importance: 0.5,
            },
            Source {
                identity,
                agent: None,
                thread: None,
            },
            None,
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn an_agent_remembers_supersedes_and_forgets_but_never_touches_a_persons_facts() {
    let fx = Fixture::new(91, Default::default(), all_tools()).await;
    let memory = memory(&fx);
    // No bank, no memory tools.
    assert!(names(&fx.tools().await).is_empty());
    let own = bank(&memory, "operator", "shared", Some(fx.agent)).await;
    assert_eq!(
        names(&fx.tools().await),
        [
            "memory.recall",
            "memory.browse",
            "memory.write",
            "memory.supersede",
            "memory.forget"
        ]
    );

    let written = fx
        .output(
            "memory.write",
            json!({"kind":"preferences","path":"/people/sam/","title":"Sam prefers email",
                "content":"Sam prefers email over phone calls for anything non-urgent."}),
        )
        .await;
    let first = Uuid::parse_str(written["memory_id"].as_str().unwrap()).unwrap();
    let recalled = fx
        .output(
            "memory.recall",
            json!({"query":"how should I contact Sam?"}),
        )
        .await;
    assert_eq!(titles(&recalled), ["Sam prefers email"]);
    assert_eq!(
        (
            recalled["memories"][0]["path"].clone(),
            recalled["memories"][0]["origin"].clone()
        ),
        (json!("people/sam"), json!("explicit"))
    );
    assert_eq!(recalled["memories"][0]["matched"], json!(["keywords"]));
    let row = memory.memory(first).await.unwrap();
    assert_eq!(
        (row.thread_id, row.written_by_agent_id),
        (Some(fx.thread), Some(fx.agent))
    );
    let browsed = fx.output("memory.browse", json!({})).await;
    assert_eq!(browsed["paths"], json!([{"path":"people","count":1}]));
    // An id already written is a retry only for the same write.
    let reused = memory
        .create(
            first,
            own,
            Draft {
                kind: "events".into(),
                path: String::new(),
                title: "Other".into(),
                content: "Other.".into(),
                valid_from: None,
                valid_to: None,
                ongoing: false,
                importance: 0.5,
            },
            Source {
                identity: None,
                agent: None,
                thread: None,
            },
            None,
        )
        .await;
    assert!(reused.is_err(), "a person reused an agent's memory id");

    // The fact changed: the new memory takes the old one's place, which stays in history and
    // cannot be replaced again.
    let superseded = fx
        .output(
            "memory.supersede",
            json!({"memory_id":first,"content":"Sam now prefers a Slack message over email."}),
        )
        .await;
    let second = superseded["memory_id"].as_str().unwrap().to_owned();
    let recalled = fx
        .output(
            "memory.recall",
            json!({"query":"how should I contact Sam?"}),
        )
        .await;
    assert_eq!(recalled["memories"].as_array().unwrap().len(), 1);
    assert_eq!(recalled["memories"][0]["memory_id"], json!(second));
    assert_eq!(recalled["memories"][0]["title"], json!("Sam prefers email"));
    let operations: Vec<String> = memory
        .revisions(first)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.operation)
        .collect();
    assert_eq!(operations, ["create", "supersede"]);
    assert_eq!(
        memory
            .memory(first)
            .await
            .unwrap()
            .superseded_by
            .map(|s| s.to_string()),
        Some(second.clone())
    );
    for (tool, input) in [
        (
            "memory.supersede",
            json!({"memory_id":first,"title":"again"}),
        ),
        ("memory.forget", json!({"memory_id":first})),
    ] {
        let refused = fx.invoke(tool, input).await;
        assert!(
            refused["error"]["message"]
                .as_str()
                .is_some_and(|m| m.contains("recall the current one")),
            "{tool} of a replaced memory: {refused}"
        );
    }

    // A person's fact is recalled, but no agent tool changes or forgets it.
    let fact = person_fact(&memory, own, "Deploys freeze on Fridays", None).await;
    let recalled = fx
        .output("memory.recall", json!({"query":"can I deploy on Friday?"}))
        .await;
    assert_eq!(recalled["memories"][0]["origin"], json!("owner"));
    for (tool, input) in [
        ("memory.forget", json!({"memory_id":fact})),
        (
            "memory.supersede",
            json!({"memory_id":fact,"content":"Deploy whenever."}),
        ),
    ] {
        let result = fx.invoke(tool, input).await;
        assert!(
            result.get("error").is_some(),
            "{tool} changed a person's fact: {result}"
        );
    }
    assert_eq!(
        memory.memory(fact).await.unwrap().title,
        "Deploys freeze on Fridays"
    );

    // Forgetting is retry-safe: forgetting it again succeeds.
    for _ in 0..2 {
        fx.output("memory.forget", json!({"memory_id":second}))
            .await;
    }
    let recalled = fx
        .output(
            "memory.recall",
            json!({"query":"how should I contact Sam?"}),
        )
        .await;
    assert!(titles(&recalled).is_empty(), "{recalled}");
    fx.close().await;
}

#[tokio::test]
async fn recall_without_a_query_lists_what_held_during_a_period() {
    let fx = Fixture::new(96, Default::default(), all_tools()).await;
    let memory = memory(&fx);
    bank(&memory, "operator", "shared", Some(fx.agent)).await;
    for (title, from, to) in [
        ("Office move", "2026-03-01T00:00:00Z", None),
        (
            "Hiring freeze",
            "2026-01-01T00:00:00Z",
            Some("2026-02-01T00:00:00Z"),
        ),
    ] {
        let mut input =
            json!({"kind":"events","title":title,"content":format!("{title}."),"valid_from":from});
        if let Some(to) = to {
            input["valid_to"] = json!(to);
        }
        fx.output("memory.write", input).await;
    }
    fx.output(
        "memory.write",
        json!({"kind":"entities","title":"Timeless","content":"Holds always."}),
    )
    .await;
    let all = fx.output("memory.recall", json!({})).await;
    assert_eq!(titles(&all), ["Timeless", "Hiring freeze", "Office move"]);
    // A period must end after it starts.
    let backwards = fx
        .invoke(
            "memory.recall",
            json!({"during":{"from":"2026-02-01T00:00:00Z","to":"2026-01-01T00:00:00Z"}}),
        )
        .await;
    assert_eq!(
        backwards["error"]["code"],
        json!("invalid_argument"),
        "{backwards}"
    );
    // An ongoing fact holds now; it cannot also have an end.
    let ongoing = fx
        .output(
            "memory.write",
            json!({"kind":"entities","title":"Remote Fridays","content":"Fridays are remote.",
                "valid_from":"2026-04-01T00:00:00Z","ongoing":true}),
        )
        .await;
    assert!(ongoing.get("memory_id").is_some(), "{ongoing}");
    let now = fx
        .output(
            "memory.recall",
            json!({"during":{"from":chrono::Utc::now().to_rfc3339()}}),
        )
        .await;
    assert_eq!(titles(&now), ["Remote Fridays"]);
    assert_eq!(now["memories"][0]["ongoing"], json!(true));
    assert!(now["memories"][0].get("valid_to").is_none(), "{now}");
    // Correcting only its start keeps it ongoing.
    fx.output(
        "memory.supersede",
        json!({"memory_id":ongoing["memory_id"],"valid_from":"2026-03-01T00:00:00Z"}),
    )
    .await;
    let now = fx
        .output(
            "memory.recall",
            json!({"during":{"from":chrono::Utc::now().to_rfc3339()}}),
        )
        .await;
    assert_eq!(titles(&now), ["Remote Fridays"]);
    assert_eq!(now["memories"][0]["ongoing"], json!(true));
    assert_eq!(
        now["memories"][0]["valid_from"],
        json!("2026-03-01T00:00:00Z")
    );
    let both = fx
        .invoke(
            "memory.write",
            json!({"kind":"entities","title":"x","content":"y","valid_from":"2026-04-01T00:00:00Z",
                "valid_to":"2026-05-01T00:00:00Z","ongoing":true}),
        )
        .await;
    assert!(both.get("error").is_some(), "{both}");
    let january = fx
        .output(
            "memory.recall",
            json!({"during":{"from":"2026-01-10T00:00:00Z","to":"2026-01-20T00:00:00Z"}}),
        )
        .await;
    assert_eq!(titles(&january), ["Hiring freeze"]);
    fx.close().await;
}

#[tokio::test]
async fn deleting_an_agent_deletes_its_own_bank_and_keeps_shared_ones() {
    let fx = Fixture::new(97, Default::default(), all_tools()).await;
    let memory = memory(&fx);
    let own = bank(&memory, "operator", "shared", Some(fx.agent)).await;
    let shared = bank(&memory, "handbook", "shared", None).await;
    memory
        .set_link(fx.agent, shared, "read_write")
        .await
        .unwrap();
    let kept = person_fact(&memory, shared, "Expenses need a receipt", None).await;
    let gone = person_fact(&memory, own, "Mine alone", None).await;
    fx.agents.pause(fx.agent).await.unwrap();
    fx.agents.delete(fx.agent).await.unwrap();
    assert!(memory.bank(own).await.is_err());
    assert!(memory.memory(gone).await.is_err());
    assert!(memory.memory(kept).await.is_ok());
    assert!(memory.bank(shared).await.unwrap().agent_ids.is_empty());
    // A deleted agent gets no banks or links.
    assert!(memory.set_link(fx.agent, shared, "read").await.is_err());
    assert!(
        memory
            .create_bank("afterlife", "", "shared", Some(fx.agent),)
            .await
            .is_err()
    );
    fx.close().await;
}

#[tokio::test]
async fn a_read_link_offers_recall_only_and_another_agents_bank_stays_its_own() {
    let fx = Fixture::new(92, Default::default(), all_tools()).await;
    let memory = memory(&fx);
    let handbook = bank(&memory, "handbook", "shared", None).await;
    person_fact(&memory, handbook, "Expenses need a receipt", None).await;
    memory.set_link(fx.agent, handbook, "read").await.unwrap();
    assert_eq!(names(&fx.tools().await), ["memory.recall", "memory.browse"]);
    let recalled = fx
        .output(
            "memory.recall",
            json!({"query":"do I need a receipt for expenses?"}),
        )
        .await;
    assert_eq!(titles(&recalled), ["Expenses need a receipt"]);
    // Query terms are stemmed once: "database" still finds "Database" without embeddings.
    person_fact(&memory, handbook, "Database backups run nightly", None).await;
    let recalled = fx
        .output("memory.recall", json!({"query":"database"}))
        .await;
    assert_eq!(titles(&recalled), ["Database backups run nightly"]);
    let refused = fx
        .invoke(
            "memory.write",
            json!({"kind":"events","title":"x","content":"y"}),
        )
        .await;
    assert!(refused.get("error").is_some(), "{refused}");

    // Another agent's own bank cannot be linked to this one.
    let other = common::invocation::create_agent(&fx.agents, "Other", Default::default()).await;
    let theirs = bank(&memory, "other", "shared", Some(other)).await;
    assert!(memory.set_link(fx.agent, theirs, "read").await.is_err());
    assert!(memory.remove_link(other, theirs).await.is_err());
    fx.close().await;
}

#[tokio::test]
async fn a_per_person_bank_keeps_each_persons_memories_apart() {
    let mut fx = Fixture::new(93, Default::default(), all_tools()).await;
    let memory = memory(&fx);
    let people = bank(&memory, "people", "per_person", None).await;
    memory
        .set_link(fx.agent, people, "read_write")
        .await
        .unwrap();
    person_fact(&memory, people, "Office closes at six", None).await;
    let write = |title: &str| json!({"kind":"profile","title":title,"content":format!("{title}.")});

    fx.output("memory.write", write("Alex is vegetarian")).await;
    let alex = fx.user_id.clone();
    let blake = fx.chat.create_user("Blake").await.unwrap().id;
    fx.user_id = blake;
    fx.next_invocation().await;
    let recalled = fx
        .output(
            "memory.recall",
            json!({"query":"is anyone vegetarian, and when does the office close?"}),
        )
        .await;
    assert_eq!(titles(&recalled), ["Office closes at six"]);
    fx.output("memory.write", write("Blake is allergic to nuts"))
        .await;

    fx.user_id = alex;
    fx.next_invocation().await;
    let mut seen = titles(
        &fx.output(
            "memory.recall",
            json!({"query":"is anyone vegetarian or allergic, and when does the office close?"}),
        )
        .await,
    )
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    seen.sort();
    assert_eq!(seen, ["Alex is vegetarian", "Office closes at six"]);
    // A person managing the bank sees everyone's.
    let all = memory
        .recall(
            &[people],
            Some("is anyone vegetarian or allergic, and when does the office close?"),
            &Filter::default(),
            Person::Everyone,
            10,
        )
        .await
        .unwrap();
    assert_eq!(all.len(), 3);

    // The bank page: who it keeps memories for, narrowed to one of them or to people's facts,
    // a page at a time, and the paths it holds.
    let listed = memory.people(people).await.unwrap();
    let blake_row = listed.iter().find(|p| p.name == "Blake").unwrap();
    assert_eq!((listed.len(), blake_row.count), (2, 1));
    let bank_memory = &memory;
    let only = |filter: Filter| async move {
        bank_memory
            .list(&[people], &filter, Person::Everyone, None, 10)
            .await
            .unwrap()
            .into_iter()
            .map(|m| m.title)
            .collect::<Vec<_>>()
    };
    let blakes = Filter {
        identity: Some(blake_row.identity_id),
        ..Default::default()
    };
    assert_eq!(only(blakes).await, ["Blake is allergic to nuts"]);
    let owners = Filter {
        origin: Some("owner".into()),
        ..Default::default()
    };
    assert_eq!(only(owners).await, ["Office closes at six"]);
    let mut paged = Vec::new();
    let mut after = None;
    loop {
        let page = memory
            .list(&[people], &Filter::default(), Person::Everyone, after, 1)
            .await
            .unwrap();
        let Some(last) = page.last() else { break };
        after = Some(last.id);
        paged.push(last.title.clone());
    }
    assert_eq!(paged.len(), 3, "{paged:?}");
    let paths: Vec<_> = memory
        .paths(people)
        .await
        .unwrap()
        .into_iter()
        .map(|p| (p.path, p.count))
        .collect();
    assert_eq!(paths, [(String::new(), 2), ("team".to_owned(), 1)]);
    fx.close().await;
}

/// An OpenAI-compatible embeddings endpoint that places texts by topic, so similarity follows
/// meaning rather than shared words.
async fn embeddings() -> String {
    let app = axum::Router::new().route(
        "/v1/embeddings",
        axum::routing::post(|axum::Json(body): axum::Json<Value>| async move {
            let topic = |text: &str| {
                let text = text.to_lowercase();
                if ["dog", "puppy", "canine"].iter().any(|w| text.contains(w)) {
                    0
                } else if ["invoice", "billing"].iter().any(|w| text.contains(w)) {
                    1
                } else {
                    2
                }
            };
            let data: Vec<Value> = body["input"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| {
                    let mut vector = vec![0.01f32; 1536];
                    vector[topic(t.as_str().unwrap())] = 1.0;
                    json!({"embedding": vector})
                })
                .collect();
            axum::Json(json!({"data": data}))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}
/// A ready OpenAI connection, set up through the broker as the setup page does.
async fn openai(connections: &Connections, base_url: &str) -> Uuid {
    let id = Uuid::new_v4();
    let started = connections
        .start(id, "OpenAI", "openai", "api", &[])
        .await
        .unwrap();
    let url = url::Url::parse(&started.brokering_url).unwrap();
    let setup = Uuid::parse_str(url.path_segments().unwrap().next_back().unwrap()).unwrap();
    let token = url
        .query_pairs()
        .find(|(k, _)| k == "connection_setup_token")
        .unwrap()
        .1
        .into_owned();
    let view = connections.view(setup, &token).await.unwrap();
    let view = connections
        .set_connection_name(setup, &token, view.action_id, "embeddings")
        .await
        .unwrap();
    connections
        .advance(
            setup,
            &token,
            view.action_id,
            [
                ("api_key".to_owned(), SecretString::from("sk-test")),
                ("base_url".to_owned(), SecretString::from(base_url)),
            ]
            .into_iter()
            .collect(),
        )
        .await
        .unwrap();
    id
}

#[tokio::test]
async fn recall_finds_memories_by_meaning_once_the_worker_has_embedded_them() {
    let fx = Fixture::new(94, Default::default(), all_tools()).await;
    let memory = memory(&fx);
    let own = bank(&memory, "operator", "shared", Some(fx.agent)).await;
    for (title, content) in [
        ("Rex", "We adopted a puppy named Rex in March."),
        ("Acme terms", "Acme pays each invoice within 30 days."),
    ] {
        fx.output(
            "memory.write",
            json!({"kind":"entities","title":title,"content":content}),
        )
        .await;
    }
    // Without settings nothing is embedded, and a query sharing no words finds nothing.
    assert_eq!(memory.embed_pending().await.unwrap(), 0);
    assert!(
        titles(
            &fx.output("memory.recall", json!({"query":"canine companion"}))
                .await
        )
        .is_empty()
    );

    // Nothing waits for an embedding until a model is set.
    assert_eq!(memory.bank(own).await.unwrap().pending_embeddings, 0);
    let url = embeddings().await;
    let connection = openai(&fx.connections, &url).await;
    // A model answering with another dimension is refused before anything changes.
    assert!(
        memory
            .set_settings(connection, "text-embedding-3-small", 1024)
            .await
            .is_err()
    );
    memory
        .set_settings(connection, "text-embedding-3-small", 1536)
        .await
        .unwrap();
    assert_eq!(memory.bank(own).await.unwrap().pending_embeddings, 2);
    assert_eq!(memory.embed_pending().await.unwrap(), 2);
    assert_eq!(memory.bank(own).await.unwrap().pending_embeddings, 0);

    let recalled = fx
        .output("memory.recall", json!({"query":"canine companion"}))
        .await;
    assert_eq!(titles(&recalled)[0], "Rex");
    let recalled = fx
        .output("memory.recall", json!({"query":"billing schedule"}))
        .await;
    assert_eq!(titles(&recalled)[0], "Acme terms");

    // New settings ask for every embedding again; full text still answers meanwhile.
    memory
        .set_settings(connection, "text-embedding-3-small", 1536)
        .await
        .unwrap();
    assert_eq!(memory.bank(own).await.unwrap().pending_embeddings, 2);
    assert_eq!(
        titles(&fx.output("memory.recall", json!({"query":"puppy"})).await),
        ["Rex"]
    );
    fx.close().await;
}
