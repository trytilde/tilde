mod common;
use buffa::Message;
use common::{Database, seed};
use secrecy::{ExposeSecret, SecretString};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tilde::{
    agent::{Agents, CreateAgent},
    chat::{self, Chat, Scope},
    encryption::Encryption,
    iam::capabilities::{Capabilities, Capability, Reach},
    proto::tilde::types::v1 as types,
};
use uuid::Uuid;

struct Fixture {
    db: Database,
    chat: Chat,
    scope: Scope,
    token: SecretString,
    user: Uuid,
}
impl Fixture {
    async fn new() -> Self {
        let db = Database::new().await;
        let encryption = Arc::new(Encryption::initialize(&db.pool, seed(81)).await.unwrap());
        let agent = Agents::new(db.pool.clone(), encryption.clone())
            .create(CreateAgent {
                id: Uuid::new_v4(),
                name: "Batch fixture".into(),
                concurrency_policy: Default::default(),
                capabilities: Capabilities(BTreeMap::from([
                    (Capability::ToolsInvoke, Reach::All),
                    (Capability::ThreadRead, Reach::Yes),
                ])),
            })
            .await
            .unwrap();
        let chat = Chat::new(db.pool.clone(), encryption, "http://127.0.0.1:1".into());
        let user = chat.create_user("Caller").await.unwrap();
        let thread = chat
            .create_thread(chat::CreateThread {
                title: "Batch fixture".into(),
                primary_agent_id: agent.id.to_string(),
                participants: vec![
                    types::ParticipantRef {
                        agent_id: Some(agent.id.to_string()),
                        ..Default::default()
                    },
                    types::ParticipantRef {
                        user_id: Some(user.id.clone()),
                        ..Default::default()
                    },
                ],
            })
            .await
            .unwrap();
        let user = Uuid::parse_str(
            &thread
                .participants
                .iter()
                .find(|p| p.user_id.as_deref() == Some(&user.id))
                .unwrap()
                .id,
        )
        .unwrap();
        let run = chat
            .start_run(chat::StartRun {
                thread_id: thread.id.clone(),
                agent_id: agent.id.to_string(),
                objective: "Batch fixture".into(),
                idempotency_key: "batch".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        let invocation = Uuid::parse_str(&run.invocation_id).unwrap();
        db.pool.get().await.unwrap().execute("UPDATE chat_invocations SET status='running',lease_expires_at=NOW()+INTERVAL '5 minutes' WHERE id=$1", &[&invocation]).await.unwrap();
        let token = chat
            .tokens
            .issue(
                agent.id,
                invocation,
                Uuid::parse_str(&thread.id).unwrap(),
                Uuid::parse_str(&run.id).unwrap(),
            )
            .await
            .unwrap();
        let scope = chat.scope(token.expose_secret()).await.unwrap();
        chat::audit::flush(&db.pool).await.unwrap();
        Self {
            db,
            chat,
            scope,
            token,
            user,
        }
    }
}

#[tokio::test]
async fn audit_batches_wait_for_commit_discard_rollback_and_recover_missing_sequences() {
    let fx = Fixture::new().await;
    // Audit INSERTs hold KEY SHARE for their thread foreign key. They must not block
    // canonical updates that leave the thread's primary key unchanged.
    let mut foreign_key_reader_client = fx.db.pool.get().await.unwrap();
    let foreign_key_reader = foreign_key_reader_client.transaction().await.unwrap();
    foreign_key_reader
        .execute(
            "SELECT id FROM chat_threads WHERE id=$1 FOR KEY SHARE",
            &[&fx.scope.thread_id],
        )
        .await
        .unwrap();
    tokio::time::timeout(
        Duration::from_secs(2),
        fx.chat.typing(fx.scope.thread_id, fx.user, false),
    )
    .await
    .unwrap()
    .unwrap();
    foreign_key_reader.rollback().await.unwrap();
    drop(foreign_key_reader_client);
    // Blocking history INSERTs must not block a committed canonical typing update.
    let mut history_lock_client = fx.db.pool.get().await.unwrap();
    let history_lock = history_lock_client.transaction().await.unwrap();
    history_lock
        .execute("LOCK TABLE chat_activity IN SHARE MODE", &[])
        .await
        .unwrap();
    tokio::time::timeout(
        Duration::from_secs(2),
        fx.chat.typing(fx.scope.thread_id, fx.user, true),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        fx.db
            .pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM chat_typing WHERE thread_id=$1 AND participant_id=$2)",
                &[&fx.scope.thread_id, &fx.user]
            )
            .await
            .unwrap()
            .get::<_, bool>(0)
    );
    history_lock.rollback().await.unwrap();
    drop(history_lock_client);
    chat::audit::flush(&fx.db.pool).await.unwrap();

    let entity = Uuid::new_v4();
    let event = |text: &str| types::Activity {
        kind: "reasoning.delta".into(),
        entity_id: entity.to_string(),
        text_delta: text.into(),
        ..Default::default()
    };
    let mut tx_client = fx.db.pool.get().await.unwrap();
    let tx = tx_client.transaction().await.unwrap();
    chat::audit::append(&fx.db.pool, &tx, fx.scope.thread_id, event("rollback"))
        .await
        .unwrap();
    chat::audit::flush(&fx.db.pool).await.unwrap();
    assert_eq!(
        fx.db
            .pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT COUNT(*) FROM chat_activity WHERE entity_id=$1",
                &[&entity]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    tx.rollback().await.unwrap();
    drop(tx_client);
    let mut tx_client = fx.db.pool.get().await.unwrap();
    let tx = tx_client.transaction().await.unwrap();
    for text in ["first", "second"] {
        chat::audit::append(&fx.db.pool, &tx, fx.scope.thread_id, event(text))
            .await
            .unwrap();
    }
    tx.commit().await.unwrap();
    drop(tx_client);
    chat::audit::flush(&fx.db.pool).await.unwrap();
    let rows = fx
        .db
        .pool
        .get()
        .await
        .unwrap()
        .query(
            "SELECT sequence,snapshot FROM chat_activity WHERE entity_id=$1 ORDER BY sequence",
            &[&entity],
        )
        .await
        .unwrap()
        .into_iter()
        .map(|row| (row.get::<_, i64>(0), row.get::<_, Vec<u8>>(1)))
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 2);
    for ((sequence, bytes), text) in rows.iter().zip(["first", "second"]) {
        let snapshot = types::Activity::decode_from_slice(bytes).unwrap();
        assert_eq!(snapshot.sequence, *sequence);
        assert_eq!(snapshot.text_delta, text);
    }
    // Simulate a committed sequence whose memory-only event was lost in a crash.
    fx.db
        .pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE chat_threads SET activity_sequence=activity_sequence+1 WHERE id=$1",
            &[&fx.scope.thread_id],
        )
        .await
        .unwrap();
    let mut tx_client = fx.db.pool.get().await.unwrap();
    let tx = tx_client.transaction().await.unwrap();
    chat::audit::append(&fx.db.pool, &tx, fx.scope.thread_id, event("after gap"))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    drop(tx_client);
    chat::audit::flush(&fx.db.pool).await.unwrap();
    let page = fx
        .chat
        .activity_page(fx.scope.thread_id, rows.last().unwrap().0, 100)
        .await
        .unwrap();
    assert_eq!(page.events.len(), 1);
    assert_eq!(page.events[0].text_delta, "after gap");
    fx.db.close().await;
}

#[tokio::test]
async fn cache_and_dependencies_are_batched_atomic_and_idempotent() {
    let fx = Fixture::new().await;
    let ids: Vec<_> = (0..50).map(|_| Uuid::new_v4()).collect();
    fx.db.pool.get().await.unwrap().execute("INSERT INTO chat_messages(id,thread_id,participant_id,text,status) SELECT id,$2,$3,'message','complete' FROM UNNEST($1::UUID[]) AS id", &[&(&ids), &fx.scope.thread_id, &fx.user]).await.unwrap();
    let batch = |value: &str| {
        ids.iter()
            .map(|id| chat::ConvertedMessage {
                message_id: id.to_string(),
                message_json: json!({"value":value}).to_string(),
            })
            .collect::<Vec<_>>()
    };
    fx.chat
        .cache_converted_messages(&fx.scope, batch("original"))
        .await
        .unwrap();
    let mut invalid = batch("replacement");
    invalid.push(chat::ConvertedMessage {
        message_id: Uuid::new_v4().to_string(),
        message_json: "{}".into(),
    });
    assert!(
        fx.chat
            .cache_converted_messages(&fx.scope, invalid)
            .await
            .is_err()
    );
    let cached = fx
        .chat
        .hydrate_converted_messages(
            fx.scope.agent_id,
            fx.scope.thread_id,
            &ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
        )
        .await
        .unwrap();
    assert_eq!(cached.len(), 50);
    assert!(cached.iter().all(|m| m.message_json.contains("original")));
    let mut duplicate = batch("updated");
    duplicate.push(chat::ConvertedMessage {
        message_id: ids[0].to_string(),
        message_json: "{\"last\":true}".into(),
    });
    fx.chat
        .cache_converted_messages(&fx.scope, duplicate)
        .await
        .unwrap();
    assert_eq!(
        fx.chat
            .hydrate_converted_messages(
                fx.scope.agent_id,
                fx.scope.thread_id,
                &[ids[0].to_string()]
            )
            .await
            .unwrap()[0]
            .message_json,
        "{\"last\":true}"
    );

    fx.db.pool.get().await.unwrap().execute("INSERT INTO chat_tasks(id,thread_id,agent_id,title) SELECT id,$2,$3,'dependency' FROM UNNEST($1::UUID[]) AS id", &[&(&ids), &fx.scope.thread_id, &fx.scope.agent_id]).await.unwrap();
    let task_id = Uuid::new_v4();
    let create = || chat::CreateTask {
        id: task_id.to_string(),
        title: "Dependent".into(),
        dependency_ids: ids.iter().map(ToString::to_string).collect(),
        ..Default::default()
    };
    let (first, retry) = tokio::join!(
        fx.chat.create_task(&fx.scope, create()),
        fx.chat.create_task(&fx.scope, create())
    );
    assert_eq!(first.unwrap().dependency_ids.len(), 50);
    assert_eq!(retry.unwrap().dependency_ids.len(), 50);
    let invalid_id = Uuid::new_v4();
    let mut invalid = create();
    invalid.id = invalid_id.to_string();
    invalid.dependency_ids.push(Uuid::new_v4().to_string());
    assert!(fx.chat.create_task(&fx.scope, invalid).await.is_err());
    assert!(
        !fx.db
            .pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM chat_tasks WHERE id=$1)",
                &[&invalid_id]
            )
            .await
            .unwrap()
            .get::<_, bool>(0)
    );
    fx.db.close().await;
}

fn frame(value: Value) -> Vec<u8> {
    let bytes = serde_json::to_vec(&value).unwrap();
    let mut result = vec![0];
    result.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    result.extend(bytes);
    result
}
fn responses(mut bytes: &[u8]) -> Vec<Value> {
    let mut result = vec![];
    while !bytes.is_empty() {
        let n = u32::from_be_bytes(bytes[1..5].try_into().unwrap()) as usize;
        result.push(serde_json::from_slice(&bytes[5..5 + n]).unwrap());
        bytes = &bytes[5 + n..];
    }
    result
}

#[tokio::test]
async fn complete_native_messages_preserve_snapshots_and_execute_a_call_only_once() {
    let fx = Fixture::new().await;
    let file = fx
        .chat
        .upload_attachment(chat::UploadAttachment {
            id: Uuid::new_v4().to_string(),
            thread_id: fx.scope.thread_id.to_string(),
            filename: "note.txt".into(),
            media_type: "text/plain".into(),
            content: b"note".to_vec(),
        })
        .await
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "http://{}/tilde.runtime.v1.ChatService/InvokeTool",
        listener.local_addr().unwrap()
    );
    let server = tokio::spawn(
        axum::serve(listener, chat::rpc::runtime::router(fx.chat.clone())).into_future(),
    );
    let http = reqwest::Client::new();
    let invoke = |frames: Vec<Value>| {
        http.post(&url)
            .bearer_auth(fx.token.expose_secret())
            .header("content-type", "application/connect+json")
            .header("connect-protocol-version", "1")
            .body(frames.into_iter().flat_map(frame).collect::<Vec<_>>())
            .send()
    };
    let call = Uuid::new_v4();
    let send = |id: Uuid, attachment: String| {
        invoke(vec![
            json!({"name":"sendMessage","callId":id,"sequence":0,"finish":true,
            "inputJson":json!({"text":"complete text","attachmentIds":[attachment],"addressedParticipantIds":[fx.user]}).to_string()}),
        ])
    };
    let (a, b) = tokio::join!(send(call, file.id.clone()), send(call, file.id.clone()));
    let a = responses(&a.unwrap().bytes().await.unwrap());
    let b = responses(&b.unwrap().bytes().await.unwrap());
    assert_ne!(
        a.last().unwrap().get("error").is_some(),
        b.last().unwrap().get("error").is_some()
    );
    let message = fx.chat.message(call).await.unwrap();
    assert_eq!(message.text, "complete text");
    assert_eq!(message.status, "complete");
    assert_eq!(message.attachments[0].id, file.id);
    assert_eq!(message.addressed_participant_ids, vec![fx.user.to_string()]);
    let stored = {
        let row = fx
            .db
            .pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT status,output_json FROM chat_tool_calls WHERE id=$1",
                &[&call],
            )
            .await
            .unwrap();
        (row.get::<_, String>(0), row.get::<_, String>(1))
    };
    assert_eq!(stored.0, "completed");
    let successful_response = if a.last().unwrap().get("error").is_none() {
        &a
    } else {
        &b
    };
    assert_eq!(
        successful_response[0]["outputJson"].as_str().unwrap(),
        stored.1
    );
    assert_eq!(
        serde_json::from_str::<Value>(&stored.1).unwrap(),
        serde_json::to_value(&message).unwrap()
    );
    chat::audit::flush(&fx.db.pool).await.unwrap();
    let events: Vec<_> = fx
        .chat
        .activity_page(fx.scope.thread_id, 0, 100)
        .await
        .unwrap()
        .events
        .into_iter()
        .filter(|e| e.entity_id == call.to_string())
        .collect();
    assert_eq!(
        events.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(),
        vec![
            "tool.started",
            "message.started",
            "message.attachments",
            "message.delta",
            "message.completed",
            "tool.completed"
        ]
    );
    let Some(types::activity::Detail::Message(started)) = &events[1].detail else {
        panic!("message snapshot")
    };
    assert!(started.text.is_empty() && started.attachments.is_empty());
    assert_eq!(started.status, "streaming");
    let Some(types::activity::Detail::Message(completed)) = &events[4].detail else {
        panic!("message snapshot")
    };
    assert_eq!(completed.attachments[0].id, file.id);
    assert_eq!(completed.text, "complete text");
    let Some(types::activity::Detail::ToolCall(completed_tool)) = &events[5].detail else {
        panic!("tool snapshot")
    };
    assert_eq!(completed_tool.output_json, stored.1);
    assert!(
        events
            .windows(2)
            .all(|e| e[1].sequence == e[0].sequence + 1)
    );
    let attachment_only = fx
        .chat
        .upload_attachment(chat::UploadAttachment {
            id: Uuid::new_v4().to_string(),
            thread_id: fx.scope.thread_id.to_string(),
            filename: "only.txt".into(),
            media_type: "text/plain".into(),
            content: b"only".to_vec(),
        })
        .await
        .unwrap();
    for input in [
        json!({"text":"reply","inReplyToMessageId":call,
            "addressedParticipantIds":[fx.user,fx.scope.participant_id]}),
        json!({"text":"","attachmentIds":[attachment_only.id]}),
    ] {
        let id = Uuid::new_v4();
        let response = invoke(vec![json!({"name":"sendMessage","callId":id,"sequence":0,
            "finish":true,"inputJson":input.to_string()})])
        .await
        .unwrap();
        let frames = responses(&response.bytes().await.unwrap());
        assert!(frames.last().unwrap().get("error").is_none(), "{frames:?}");
        let output = frames[0]["outputJson"].as_str().unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(output).unwrap(),
            serde_json::to_value(fx.chat.message(id).await.unwrap()).unwrap()
        );
        assert_eq!(
            fx.db
                .pool
                .get()
                .await
                .unwrap()
                .query_one(
                    "SELECT output_json FROM chat_tool_calls WHERE id=$1",
                    &[&id]
                )
                .await
                .unwrap()
                .get::<_, String>(0),
            output
        );
    }
    let bad = Uuid::new_v4();
    let response = send(bad, Uuid::new_v4().to_string()).await.unwrap();
    assert!(
        responses(&response.bytes().await.unwrap())
            .last()
            .unwrap()
            .get("error")
            .is_some()
    );
    assert!(fx.chat.message(bad).await.is_err());
    assert_eq!(
        fx.db
            .pool
            .get()
            .await
            .unwrap()
            .query_one("SELECT status FROM chat_tool_calls WHERE id=$1", &[&bad])
            .await
            .unwrap()
            .get::<_, String>(0),
        "failed"
    );
    // Both SQL constraint failures and validation after the write roll back the whole
    // message transaction. Failure recording must not leave a completed tool or message.
    let mut failed = vec![bad];
    for input in [
        json!({"text":"bad recipient","addressedParticipantIds":[Uuid::new_v4()]}),
        json!({"text":"bad reply","inReplyToMessageId":Uuid::new_v4()}),
        json!({"text":"reused attachment","attachmentIds":[file.id]}),
        json!({"text":"duplicate recipient","addressedParticipantIds":[fx.user,fx.user]}),
        json!({"text":""}),
        json!({"text":42}),
    ] {
        let id = Uuid::new_v4();
        let response = invoke(vec![json!({"name":"sendMessage","callId":id,"sequence":0,
            "finish":true,"inputJson":input.to_string()})])
        .await
        .unwrap();
        assert!(
            responses(&response.bytes().await.unwrap())
                .last()
                .unwrap()
                .get("error")
                .is_some()
        );
        assert!(fx.chat.message(id).await.is_err());
        assert_eq!(
            fx.db
                .pool
                .get()
                .await
                .unwrap()
                .query_one("SELECT status FROM chat_tool_calls WHERE id=$1", &[&id])
                .await
                .unwrap()
                .get::<_, String>(0),
            "failed"
        );
        failed.push(id);
    }
    chat::audit::flush(&fx.db.pool).await.unwrap();
    let history = fx
        .chat
        .activity_page(fx.scope.thread_id, 0, 100)
        .await
        .unwrap();
    for id in failed {
        assert_eq!(
            history
                .events
                .iter()
                .filter(|e| e.entity_id == id.to_string())
                .map(|e| e.kind.as_str())
                .collect::<Vec<_>>(),
            ["tool.started", "tool.failed"]
        );
    }
    // A previously claimed running call must not be taken over by the atomic path.
    let claimed = Uuid::new_v4();
    fx.chat
        .report_tool_call(
            &fx.scope,
            types::ToolCall {
                id: claimed.to_string(),
                name: "sendMessage".into(),
                provider_id: "native".into(),
                status: "running".into(),
                input_json: "{\"text\":\"claimed\"}".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let response = invoke(vec![
        json!({"name":"sendMessage","callId":claimed,"sequence":0,
        "finish":true,"inputJson":"{\"text\":\"claimed\"}"}),
    ])
    .await
    .unwrap();
    assert!(
        responses(&response.bytes().await.unwrap())
            .last()
            .unwrap()
            .get("error")
            .is_some()
    );
    assert!(fx.chat.message(claimed).await.is_err());
    assert_eq!(
        fx.db
            .pool
            .get()
            .await
            .unwrap()
            .query_one(
                "SELECT status FROM chat_tool_calls WHERE id=$1",
                &[&claimed]
            )
            .await
            .unwrap()
            .get::<_, String>(0),
        "running"
    );

    // Streaming keeps its incremental lifecycle, including aborting unfinished input.
    for finish in [true, false] {
        let id = Uuid::new_v4();
        let mut frames = vec![
            json!({"name":"sendMessage","callId":id,"sequence":0,"inputJson":"{\"text\":\"\"}"}),
            json!({"name":"sendMessage","callId":id,"sequence":1,"chunkJson":"{\"textDelta\":\"streamed\"}"}),
        ];
        if finish {
            frames.push(json!({"name":"sendMessage","callId":id,"sequence":2,"finish":true}));
        }
        let response = invoke(frames).await.unwrap();
        assert_eq!(
            responses(&response.bytes().await.unwrap())
                .last()
                .unwrap()
                .get("error")
                .is_none(),
            finish
        );
        let message = fx.chat.message(id).await.unwrap();
        assert_eq!(message.text, "streamed");
        assert_eq!(message.status, if finish { "complete" } else { "aborted" });
        assert_eq!(
            fx.db
                .pool
                .get()
                .await
                .unwrap()
                .query_one("SELECT status FROM chat_tool_calls WHERE id=$1", &[&id])
                .await
                .unwrap()
                .get::<_, String>(0),
            if finish { "completed" } else { "failed" }
        );
    }
    server.abort();
    fx.db.close().await;
}
