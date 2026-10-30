//! Memory synthesis end to end: a conversation of an agent that auto-retains into a bank, and the
//! pages of a wiki linked to it, become memories through the bank's chat model (a fake
//! OpenAI-compatible endpoint), with evidence, without touching people's facts, once per batch.
mod common;
use common::invocation::{Fixture, all_tools};
use secrecy::SecretString;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tilde::connections::service::Connections;
use tilde::memory::{Actor, Draft, Memory, Person, Source};
use tilde::wikis::{Body, Write};
use uuid::Uuid;

fn memory(fx: &Fixture) -> Memory {
    fx.chat.tools.as_ref().unwrap().memory.clone()
}
async fn bank(memory: &Memory, name: &str, scope: &str) -> Uuid {
    memory.create_bank(name, "", scope, None).await.unwrap().id
}
/// An OpenAI-compatible chat endpoint answering a probe with no operations and everything else
/// with the next scripted answer (`HTTP <status> <code>` answers with that status and error
/// code); it keeps the prompts it was sent.
struct Model {
    url: String,
    answers: Arc<Mutex<VecDeque<String>>>,
    prompts: Arc<Mutex<Vec<String>>>,
}
async fn model() -> Model {
    let answers: Arc<Mutex<VecDeque<String>>> = Arc::default();
    let prompts: Arc<Mutex<Vec<String>>> = Arc::default();
    let (a, p) = (answers.clone(), prompts.clone());
    let app = axum::Router::new().route(
        "/v1/chat/completions",
        axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
            let (answers, prompts) = (a.clone(), p.clone());
            async move {
                assert_eq!(body["max_completion_tokens"], json!(16000));
                let prompt = body["messages"][1]["content"].as_str().unwrap().to_owned();
                let answer = if prompt.starts_with("Reply with exactly") {
                    r#"{"operations": []}"#.to_owned()
                } else {
                    prompts.lock().unwrap().push(prompt);
                    answers.lock().unwrap().pop_front().expect("an answer")
                };
                if let Some(error) = answer.strip_prefix("HTTP ") {
                    let (status, code) = error.split_once(' ').unwrap_or((error, ""));
                    return (
                        axum::http::StatusCode::from_u16(status.parse().unwrap()).unwrap(),
                        axum::Json(json!({"error": {"code": code}})),
                    );
                }
                (
                    axum::http::StatusCode::OK,
                    axum::Json(json!({"choices": [{"message": {"content": answer}}]})),
                )
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Model {
        url,
        answers,
        prompts,
    }
}
impl Model {
    fn answer(&self, operations: Value) {
        self.answers
            .lock()
            .unwrap()
            .push_back(json!({ "operations": operations }).to_string());
    }
    fn raw(&self, answer: &str) {
        self.answers.lock().unwrap().push_back(answer.into());
    }
    fn prompt(&self) -> String {
        self.prompts.lock().unwrap().pop().expect("a prompt")
    }
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
        .set_connection_name(setup, &token, view.action_id, "synthesis")
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
/// A message in `thread` from a person (`user`) or the agent, `minutes` ago (plus the 20 a
/// conversation waits to be idle), with `status`.
async fn message(
    fx: &Fixture,
    thread: Uuid,
    user: Option<&str>,
    text: &str,
    minutes: f64,
    status: &str,
) -> Uuid {
    let id = Uuid::new_v4();
    let (column, who) = match user {
        Some(user) => ("user_id", Uuid::parse_str(user).unwrap()),
        None => ("agent_id", fx.agent),
    };
    fx.db
        .pool
        .get()
        .await
        .unwrap()
        .execute(
            &format!(
                "INSERT INTO chat_messages(id,thread_id,participant_id,text,status,created_at)
                 SELECT $1,$2,id,$3,$4,NOW()-make_interval(secs=>$5)
                 FROM chat_participants WHERE thread_id=$2 AND {column}=$6"
            ),
            &[
                &id,
                &thread,
                &text,
                &status,
                &((minutes + 20.0) * 60.0),
                &who,
            ],
        )
        .await
        .unwrap();
    id
}
/// Ends the agent's invocations in `thread`, so the conversation is idle.
async fn idle(fx: &Fixture, thread: Uuid) {
    fx.db
        .pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE chat_invocations SET status='stopped' WHERE thread_id=$1",
            &[&thread],
        )
        .await
        .unwrap();
}
/// Ages every page past the quiet period a page waits after a change.
async fn settle_pages(fx: &Fixture) {
    fx.db
        .pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE wiki_pages SET updated_at=updated_at-interval '6 minutes'",
            &[],
        )
        .await
        .unwrap();
}
async fn person_fact(memory: &Memory, bank: Uuid, title: &str, content: &str) -> Uuid {
    memory
        .create(
            Uuid::new_v4(),
            bank,
            Draft {
                kind: "preferences".into(),
                path: "office".into(),
                title: title.into(),
                content: content.into(),
                valid_from: None,
                valid_to: None,
                ongoing: false,
                importance: 0.5,
            },
            Source {
                identity: None,
                actor: Actor::Person,
                thread: None,
            },
            None,
        )
        .await
        .unwrap()
}
fn write(title: &str, evidence: &[&str]) -> Value {
    json!({"op": "write", "kind": "preferences", "path": "people/sam", "title": title,
        "content": format!("{title}."), "evidence": evidence})
}
async fn titles(memory: &Memory, bank: Uuid) -> Vec<String> {
    let mut titles: Vec<String> = memory
        .list(&[bank], &Default::default(), Person::Everyone, None, 50)
        .await
        .unwrap()
        .into_iter()
        .map(|m| m.title)
        .collect();
    titles.sort();
    titles
}
/// Conversations and pages waiting for the bank.
async fn pending(memory: &Memory, bank: Uuid) -> (i64, i64) {
    memory
        .synthesis_pending(bank)
        .await
        .unwrap()
        .iter()
        .fold((0, 0), |(c, p), r| match r.wiki_id {
            Some(_) => (c, p + r.count),
            None => (c + r.count, p),
        })
}
async fn run(memory: &Memory, bank: Uuid) {
    memory.run_synthesis_now(bank).await.unwrap();
    assert!(memory.synthesize_pending().await.unwrap());
}

#[tokio::test]
async fn a_conversation_becomes_memories_once_and_never_touches_peoples_facts() {
    let fx = Fixture::new(95, Default::default(), all_tools()).await;
    let memory = memory(&fx);
    let team = bank(&memory, "team", "shared").await;
    let user = fx.user_id.clone();
    // History from before retaining starts is never synthesized.
    message(
        &fx,
        fx.thread,
        Some(&user),
        "Old news: I like tea.",
        10.0,
        "complete",
    )
    .await;
    // Only a read-write link feeds a bank.
    memory.set_link(fx.agent, team, "read").await.unwrap();
    assert!(memory.set_retain(fx.agent, team, true).await.is_err());
    memory.set_link(fx.agent, team, "read_write").await.unwrap();
    memory.set_retain(fx.agent, team, true).await.unwrap();
    let office = person_fact(&memory, team, "Office hours", "The office closes at six.").await;

    let model = model().await;
    let connection = openai(&fx.connections, &model.url).await;
    memory
        .set_synthesis(team, connection, "gpt-test", "Keep what people prefer.")
        .await
        .unwrap();
    // Nothing waits yet: the bank is claimed and released.
    assert!(memory.synthesize_pending().await.unwrap());
    assert!(memory.batches(team).await.unwrap().0.is_empty());

    let said = message(
        &fx,
        fx.thread,
        Some(&user),
        "I'm Sam. I prefer email, and the office closes at five now.",
        5.0,
        "complete",
    )
    .await;
    message(&fx, fx.thread, None, "Noted, Sam.", 4.9, "complete").await;
    // A message still streaming holds back everything after it.
    message(&fx, fx.thread, None, "Thinking", 4.8, "streaming").await;
    message(
        &fx,
        fx.thread,
        Some(&user),
        "Also: never call me.",
        4.7,
        "complete",
    )
    .await;
    idle(&fx, fx.thread).await;
    assert_eq!(pending(&memory, team).await, (1, 0));
    model.answer(json!([
        write("Sam prefers email", &["E1"]),
        // A person's fact is theirs to change, even when shown to the model.
        {"op": "supersede", "memory_id": office, "kind": "preferences", "path": "office",
         "title": "Office hours", "content": "The office closes at five.", "evidence": ["E1"]},
        write("Made up", &["E9"]),
        {"op": "supersede", "memory_id": Uuid::new_v4(), "kind": "events", "title": "x",
         "content": "x", "evidence": ["E2"]},
        {"op": "forget", "memory_id": office, "evidence": ["E1"]},
        {"op": "rewrite"},
        {"op": "noop", "reason": "greeting"}
    ]));
    run(&memory, team).await;
    let prompt = model.prompt();
    assert!(prompt.starts_with("Now: "), "{prompt}");
    assert!(prompt.contains("Keep what people prefer."), "{prompt}");
    assert!(prompt.contains(&office.to_string()), "{prompt}");
    assert!(prompt.contains("User (person): I'm Sam."), "{prompt}");
    assert!(!prompt.contains("never call me"), "{prompt}");
    // What came before retaining started is context, never evidence.
    let (context, source) = prompt.split_once("\nSource:\n").unwrap();
    assert!(context.contains("Old news"), "{prompt}");
    assert!(!source.contains("Old news"), "{prompt}");

    let (batches, wrote) = memory.batches(team).await.unwrap();
    let batch = &batches[0];
    assert_eq!(
        (
            batch.status.as_str(),
            batch.written,
            batch.superseded,
            batch.dropped
        ),
        ("applied", 1, 0, 5),
        "{}",
        batch.notes
    );
    for why in [
        "A person wrote this memory",
        "evidence not in this batch",
        "not shown",
        "not derived from this page",
        "could not be read",
    ] {
        assert!(batch.notes.contains(why), "{why}: {}", batch.notes);
    }
    assert_eq!(
        wrote.iter().map(|w| w.title.as_str()).collect::<Vec<_>>(),
        ["Sam prefers email"]
    );
    let sam = memory.memory(wrote[0].memory_id).await.unwrap();
    assert_eq!(
        (sam.origin.as_str(), sam.written_by_agent_id),
        ("synthesized", None)
    );
    let evidence = memory
        .evidence(&[sam.id])
        .await
        .unwrap()
        .remove(&sam.id)
        .unwrap();
    assert_eq!(
        (
            evidence[0].thread_id,
            evidence[0].message_id,
            evidence[0].agent_id
        ),
        (Some(fx.thread), Some(said), Some(fx.agent))
    );
    assert_eq!(
        memory.revisions(sam.id).await.unwrap()[0].actor,
        "synthesizer"
    );
    assert!(memory.memory(office).await.unwrap().superseded_by.is_none());

    // A batch done once is never applied again, even with its watermark lost.
    fx.db
        .pool
        .get()
        .await
        .unwrap()
        .execute("DELETE FROM memory_synthesis_threads", &[])
        .await
        .unwrap();
    run(&memory, team).await;
    assert_eq!(memory.batches(team).await.unwrap().0.len(), 1);

    // Turning synthesis off stops it; the watermarks stay.
    memory.clear_synthesis(team).await.unwrap();
    assert!(!memory.synthesize_pending().await.unwrap());
    fx.close().await;
}

#[tokio::test]
async fn wiki_pages_feed_a_bank_by_what_changed_and_their_deletion_forgets_what_they_alone_said() {
    let fx = Fixture::new(96, Default::default(), all_tools()).await;
    let memory = memory(&fx);
    let team = bank(&memory, "team", "shared").await;
    let model = model().await;
    let connection = openai(&fx.connections, &model.url).await;
    memory
        .set_synthesis(team, connection, "gpt-test", "")
        .await
        .unwrap();
    let wikis = fx.chat.tools.as_ref().unwrap().wikis.clone();
    let handbook = wikis.create_wiki("Handbook", "", None).await.unwrap();
    let person = || Write {
        call: Uuid::new_v4(),
        agent: None,
        thread: None,
    };
    let page = wikis
        .create_page(
            &person(),
            handbook.id,
            "office/deploys",
            "Deploys",
            "Deploys happen on Tuesdays.\nRollbacks need a lead.",
        )
        .await
        .unwrap();
    memory.set_bank_wiki(team, handbook.id).await.unwrap();
    // A page waits five minutes after its last change.
    assert!(memory.synthesize_pending().await.unwrap());
    assert!(memory.batches(team).await.unwrap().0.is_empty());
    settle_pages(&fx).await;
    model.answer(json!([write("Deploys on Tuesdays", &["E1"])]));
    run(&memory, team).await;
    assert!(model.prompt().contains("Deploys happen on Tuesdays."));
    let tuesday = memory.batches(team).await.unwrap().1[0].memory_id;
    let evidence = memory
        .evidence(&[tuesday])
        .await
        .unwrap()
        .remove(&tuesday)
        .unwrap();
    assert_eq!(
        (
            evidence[0].page_id,
            evidence[0].revision,
            evidence[0].wiki_id,
            evidence[0].wiki_name.as_deref()
        ),
        (Some(page.id), Some(1), Some(handbook.id), Some("Handbook"))
    );

    // A new revision shows the model what changed and the memories the page gave; it may forget
    // those the page no longer supports.
    wikis
        .update_page(
            &person(),
            page.id,
            1,
            None,
            Some(Body::Replace(
                "Deploys happen on Wednesdays.\nRollbacks need a lead.".into(),
            )),
        )
        .await
        .unwrap();
    settle_pages(&fx).await;
    model.answer(json!([
        {"op": "forget", "memory_id": tuesday, "evidence": ["E1"]},
        write("Deploys on Wednesdays", &["E1"])
    ]));
    run(&memory, team).await;
    let prompt = model.prompt();
    assert!(prompt.contains("Changes since revision 1"), "{prompt}");
    assert!(prompt.contains("- Deploys happen on Tuesdays."), "{prompt}");
    assert!(
        prompt.contains("+ Deploys happen on Wednesdays."),
        "{prompt}"
    );
    assert!(!prompt.contains("+ Rollbacks"), "{prompt}");
    assert!(prompt.contains("[derived from this page]"), "{prompt}");
    assert_eq!(titles(&memory, team).await, ["Deploys on Wednesdays"]);

    // A save that changes nothing costs no model call.
    wikis
        .update_page(&person(), page.id, 2, Some("Deploys"), None)
        .await
        .unwrap();
    settle_pages(&fx).await;
    run(&memory, team).await;
    assert_eq!(pending(&memory, team).await, (0, 0));
    assert_eq!(memory.batches(team).await.unwrap().0.len(), 2);

    // An outage never passes a batch over, however long it lasts: the bank backs off and shows
    // the error.
    let flaky = wikis
        .create_page(&person(), handbook.id, "x", "X", "Nothing much.")
        .await
        .unwrap();
    settle_pages(&fx).await;
    for (attempt, outage) in ["HTTP 503", "HTTP 401", "HTTP 503", "HTTP 400"]
        .into_iter()
        .enumerate()
    {
        model.raw(outage);
        run(&memory, team).await;
        let state = memory.synthesis(team).await.unwrap().unwrap();
        assert_eq!(state.attempts, attempt as i32 + 1, "{outage}");
        assert!(!state.last_error.is_empty());
        assert_eq!(pending(&memory, team).await, (0, 1), "{outage}");
    }
    model.answer(json!([]));
    run(&memory, team).await;
    let (batches, _) = memory.batches(team).await.unwrap();
    assert_eq!(
        (batches[0].status.as_str(), batches[0].page_id),
        ("applied", Some(flaky.id))
    );
    assert_eq!(memory.synthesis(team).await.unwrap().unwrap().attempts, 0);

    // A batch whose content keeps failing is passed over after three tries, and the bank carries
    // on.
    wikis
        .create_page(&person(), handbook.id, "y", "Y", "Also nothing.")
        .await
        .unwrap();
    settle_pages(&fx).await;
    for (attempt, failure) in ["not json", "HTTP 400 context_length_exceeded", "HTTP 413"]
        .into_iter()
        .enumerate()
    {
        model.raw(failure);
        run(&memory, team).await;
        let state = memory.synthesis(team).await.unwrap().unwrap();
        assert!(!state.last_error.is_empty(), "{failure}");
        let waiting = if attempt < 2 { (0, 1) } else { (0, 0) };
        assert_eq!(pending(&memory, team).await, waiting, "{failure}");
    }
    let (batches, _) = memory.batches(team).await.unwrap();
    assert_eq!(batches[0].status, "skipped", "{}", batches[0].notes);

    // A page-derived memory that replaced an agent's is not the page's alone.
    let freeze = memory
        .create(
            Uuid::new_v4(),
            team,
            Draft {
                kind: "patterns".into(),
                path: "deploys".into(),
                title: "Deploy freeze".into(),
                content: "Deploys freeze in December.".into(),
                valid_from: None,
                valid_to: None,
                ongoing: false,
                importance: 0.5,
            },
            Source {
                identity: None,
                actor: Actor::Agent(fx.agent),
                thread: None,
            },
            None,
        )
        .await
        .unwrap();
    wikis
        .update_page(
            &person(),
            page.id,
            3,
            None,
            Some(Body::Replace(
                "Deploys happen on Wednesdays.\nRollbacks need a lead.\nDeploys freeze in December and January.".into(),
            )),
        )
        .await
        .unwrap();
    settle_pages(&fx).await;
    model.answer(json!([
        {"op": "supersede", "memory_id": freeze, "kind": "patterns", "path": "deploys",
         "title": "Deploy freeze", "content": "Deploys freeze in December and January.",
         "evidence": ["E1"]}
    ]));
    run(&memory, team).await;
    let prompt = model.prompt();
    assert!(prompt.contains(&freeze.to_string()), "{prompt}");
    let (batches, _) = memory.batches(team).await.unwrap();
    assert_eq!(batches[0].superseded, 1, "{}", batches[0].notes);
    // ...and stays so when the page replaces it again: the agent's fact is down the chain.
    let derived = memory.batches(team).await.unwrap().1;
    let freeze = derived
        .iter()
        .find(|m| m.batch_id == batches[0].id)
        .unwrap()
        .memory_id;
    wikis
        .update_page(
            &person(),
            page.id,
            4,
            None,
            Some(Body::Replace(
                "Deploys happen on Wednesdays.\nRollbacks need a lead.\nDeploys freeze from December to February.".into(),
            )),
        )
        .await
        .unwrap();
    settle_pages(&fx).await;
    model.answer(json!([
        {"op": "supersede", "memory_id": freeze, "kind": "patterns", "path": "deploys",
         "title": "Deploy freeze", "content": "Deploys freeze from December to February.",
         "evidence": ["E1"]}
    ]));
    run(&memory, team).await;
    let (batches, _) = memory.batches(team).await.unwrap();
    assert_eq!(batches[0].superseded, 1, "{}", batches[0].notes);

    // Deleting the page forgets the synthesized memories it alone supported, with no model call.
    wikis.delete_page(&person(), page.id, 5).await.unwrap();
    run(&memory, team).await;
    let (batches, memories) = memory.batches(team).await.unwrap();
    assert_eq!(
        (batches[0].notes.as_str(), batches[0].forgotten),
        ("The page was deleted", 1)
    );
    assert_eq!(titles(&memory, team).await, ["Deploy freeze"]);
    let kept = memory
        .recall(
            &[team],
            Some("freeze"),
            &Default::default(),
            Person::Everyone,
            5,
        )
        .await
        .unwrap();
    assert_eq!(
        kept[0].0.content,
        "Deploys freeze from December to February."
    );
    // The batch lists what it forgot.
    assert_eq!(
        memories
            .iter()
            .filter(|m| m.batch_id == batches[0].id)
            .map(|m| (m.title.as_str(), m.forgotten))
            .collect::<Vec<_>>(),
        [("Deploys on Wednesdays", true)]
    );
    fx.close().await;
}

#[tokio::test]
async fn a_per_person_bank_learns_only_from_its_verified_persons_messages() {
    let fx = Fixture::new(97, Default::default(), all_tools()).await;
    let memory = memory(&fx);
    let people = bank(&memory, "people", "per_person").await;
    memory
        .set_link(fx.agent, people, "read_write")
        .await
        .unwrap();
    memory.set_retain(fx.agent, people, true).await.unwrap();
    let model = model().await;
    let connection = openai(&fx.connections, &model.url).await;
    memory
        .set_synthesis(people, connection, "gpt-test", "")
        .await
        .unwrap();
    // Blake was in the conversation once; their messages are not the person's.
    let blake = fx.chat.create_user("Blake").await.unwrap();
    fx.chat
        .add_participant(tilde::chat::AddParticipant {
            thread_id: fx.thread.to_string(),
            participant: Some(tilde::proto::tilde::types::v1::ParticipantRef {
                user_id: Some(blake.id.clone()),
                ..Default::default()
            }),
        })
        .await
        .unwrap();
    message(
        &fx,
        fx.thread,
        Some(&blake.id),
        "I'm Blake, I eat meat.",
        6.0,
        "complete",
    )
    .await;
    fx.db
        .pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE chat_participants SET active=FALSE WHERE thread_id=$1 AND user_id=$2",
            &[&fx.thread, &Uuid::parse_str(&blake.id).unwrap()],
        )
        .await
        .unwrap();
    message(
        &fx,
        fx.thread,
        Some(&fx.user_id),
        "I'm vegetarian.",
        5.0,
        "complete",
    )
    .await;
    idle(&fx, fx.thread).await;
    model.answer(json!([write("Vegetarian", &["E1"])]));
    run(&memory, people).await;
    let prompt = model.prompt();
    assert!(prompt.contains("I'm vegetarian."), "{prompt}");
    assert!(!prompt.contains("Blake"), "{prompt}");
    let row = memory
        .list(&[people], &Default::default(), Person::Everyone, None, 10)
        .await
        .unwrap()
        .pop()
        .unwrap();
    assert_eq!(row.identity_id, Uuid::parse_str(&fx.user_id).ok());
    let batch = &memory.batches(people).await.unwrap().0[0];
    assert!(
        batch.notes.contains("1 messages from other people"),
        "{}",
        batch.notes
    );

    // A batch with none of the person's messages asks nothing of the model.
    message(
        &fx,
        fx.thread,
        Some(&blake.id),
        "Me again.",
        4.0,
        "complete",
    )
    .await;
    run(&memory, people).await;
    let batch = &memory.batches(people).await.unwrap().0[0];
    assert_eq!(batch.status, "skipped");
    assert!(model.answers.lock().unwrap().is_empty());
    fx.close().await;
}

#[tokio::test]
async fn an_agent_starting_to_retain_leaves_what_another_agent_retains_waiting() {
    let fx = Fixture::new(98, Default::default(), all_tools()).await;
    let memory = memory(&fx);
    let team = bank(&memory, "team", "shared").await;
    memory.set_link(fx.agent, team, "read_write").await.unwrap();
    memory.set_retain(fx.agent, team, true).await.unwrap();
    message(
        &fx,
        fx.thread,
        Some(&fx.user_id),
        "I prefer email.",
        5.0,
        "complete",
    )
    .await;
    idle(&fx, fx.thread).await;
    assert_eq!(pending(&memory, team).await, (1, 0));

    // A second agent in the same conversation starts retaining: the waiting messages still wait.
    let other = common::invocation::create_agent(&fx.agents, "Other", Default::default()).await;
    fx.chat
        .add_participant(tilde::chat::AddParticipant {
            thread_id: fx.thread.to_string(),
            participant: Some(tilde::proto::tilde::types::v1::ParticipantRef {
                agent_id: Some(other.to_string()),
                ..Default::default()
            }),
        })
        .await
        .unwrap();
    memory.set_link(other, team, "read_write").await.unwrap();
    memory.set_retain(other, team, true).await.unwrap();
    assert!(pending(&memory, team).await.0 > 0);
    let model = model().await;
    let connection = openai(&fx.connections, &model.url).await;
    memory
        .set_synthesis(team, connection, "gpt-test", "")
        .await
        .unwrap();
    model.answer(json!([write("Prefers email", &["E1"])]));
    run(&memory, team).await;
    assert!(model.prompt().contains("I prefer email."));
    fx.close().await;
}

#[tokio::test]
async fn unlinking_a_wiki_keeps_its_memories_even_when_a_batch_finishes_after() {
    let fx = Fixture::new(99, Default::default(), all_tools()).await;
    let memory = memory(&fx);
    let team = bank(&memory, "team", "shared").await;
    let model = model().await;
    let connection = openai(&fx.connections, &model.url).await;
    memory
        .set_synthesis(team, connection, "gpt-test", "")
        .await
        .unwrap();
    let wikis = fx.chat.tools.as_ref().unwrap().wikis.clone();
    let handbook = wikis.create_wiki("Handbook", "", None).await.unwrap();
    let write_as_person = Write {
        call: Uuid::new_v4(),
        agent: None,
        thread: None,
    };
    let page = wikis
        .create_page(
            &write_as_person,
            handbook.id,
            "deploys",
            "Deploys",
            "Deploys happen on Tuesdays.",
        )
        .await
        .unwrap();
    memory.set_bank_wiki(team, handbook.id).await.unwrap();
    settle_pages(&fx).await;
    model.answer(json!([write("Deploys on Tuesdays", &["E1"])]));
    run(&memory, team).await;

    // The wiki is unlinked while a batch of its page is in flight; the batch then finishes and
    // marks the page, which must set no watermark, or the page would count as gone.
    memory.remove_bank_wiki(team, handbook.id).await.unwrap();
    let client = fx.db.pool.get().await.unwrap();
    assert_eq!(
        tilde::memory::db::synthesis_page_mark_execute(&client, team, page.id, 1)
            .await
            .unwrap(),
        0
    );
    run(&memory, team).await;
    assert_eq!(titles(&memory, team).await, ["Deploys on Tuesdays"]);
    assert_eq!(memory.batches(team).await.unwrap().0.len(), 1);
    fx.close().await;
}
