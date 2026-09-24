//! Cost and budgets across their boundaries: the vendored price sheet lands in Postgres and
//! prices requests at insert time, budgets settle from those costs, exhaustion withholds the
//! inference claim at token issue and renewal, and clearing the budget restores it.
mod common;
use common::{Database, seed};
use secrecy::{ExposeSecret, SecretString};
use std::sync::Arc;
use tilde::chat as application;
use tilde::proto::tilde::types::v1 as types;
use tilde::{
    agent::{Agents, CreateAgent},
    chat::Chat,
    connections::{model::*, service::Connections},
    encryption::Encryption,
    inference::{
        self,
        audit::{Record, Usage},
        budgets::{self, Action, Period, Scope},
    },
};
use uuid::Uuid;

fn record(
    agent: Uuid,
    connection: Uuid,
    run: Uuid,
    model: &str,
    input: i64,
    output: i64,
    cached: Option<i64>,
) -> Record {
    Record {
        id: Uuid::new_v4(),
        created_at: chrono::Utc::now(),
        agent_id: agent,
        connection_id: connection,
        invocation_id: Uuid::new_v4(),
        thread_id: Uuid::new_v4(),
        run_id: run,
        participant_id: Uuid::new_v4(),
        provider_id: "openai".into(),
        kind: "chat".into(),
        path: "chat/completions".into(),
        model: Some(model.into()),
        status: 200,
        latency_ms: 10,
        first_byte_ms: Some(5),
        request_bytes: 100,
        response_bytes: 200,
        input_tokens: Some(input),
        output_tokens: Some(output),
        cached_input_tokens: cached,
        cache_write_tokens: None,
        units: None,
        usage: Usage::Parsed,
    }
}

#[test]
fn the_live_sheet_is_trimmed_to_our_providers_and_units() {
    let sheet = r#"{
      "sample_spec": {"input_cost_per_token": 0.0, "litellm_provider": "one of ..."},
      "gpt-5-nano": {"input_cost_per_token": 5e-08, "output_cost_per_token": 4e-07, "cache_read_input_token_cost": 5e-09, "litellm_provider": "openai", "mode": "chat"},
      "azure/gpt-5-nano": {"input_cost_per_token": 5e-08, "output_cost_per_token": 4e-07, "litellm_provider": "azure", "mode": "chat"},
      "gemini/gemini-2.5-pro": {"input_cost_per_token": 1.25e-06, "output_cost_per_token": 1e-05, "litellm_provider": "gemini"},
      "anthropic.claude-sonnet-4-20250514-v1:0": {"input_cost_per_token": 3e-06, "output_cost_per_token": 1.5e-05, "litellm_provider": "bedrock"},
      "bedrock/anthropic.claude-sonnet-4-20250514-v1:0": {"input_cost_per_token": 3e-06, "output_cost_per_token": 1.5e-05, "litellm_provider": "bedrock_converse"},
      "openrouter/openai/gpt-5": {"input_cost_per_token": 1e-06, "litellm_provider": "openrouter"},
      "whisper-1": {"input_cost_per_second": 0.0001, "litellm_provider": "openai"}
    }"#;
    let rows = inference::prices::from_litellm(sheet).unwrap();
    let mut keys: Vec<_> = rows
        .iter()
        .map(|r| format!("{}/{}", r.provider_id, r.model))
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "azure_openai/gpt-5-nano",
            "bedrock/anthropic.claude-sonnet-4-20250514-v1:0",
            "google_ai/gemini-2.5-pro",
            "openai/gpt-5-nano",
            "openai/whisper-1",
        ]
    );
    let nano = rows
        .iter()
        .find(|r| r.model == "gpt-5-nano" && r.provider_id == "openai")
        .unwrap();
    assert_eq!(
        (
            nano.input_per_m_micros,
            nano.output_per_m_micros,
            nano.cached_input_per_m_micros
        ),
        (Some(50_000), Some(400_000), Some(5_000))
    );
    assert_eq!(
        rows.iter()
            .find(|r| r.provider_id == "azure_openai")
            .unwrap()
            .cached_input_per_m_micros,
        None
    );
    // A per-second-only model keeps its row so transcription spend can be priced.
    let whisper = rows.iter().find(|r| r.model == "whisper-1").unwrap();
    assert_eq!(
        (whisper.input_per_m_micros, whisper.second_micros),
        (None, Some(100))
    );
}

#[test]
fn bedrock_event_streams_yield_token_usage() {
    use base64::Engine;
    fn message(payload: &[u8]) -> Vec<u8> {
        let total = (16 + payload.len()) as u32;
        let mut out = total.to_be_bytes().to_vec();
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(payload);
        out.extend_from_slice(&0u32.to_be_bytes());
        out
    }
    let converse = message(
        br#"{"metadata":1,"usage":{"inputTokens":12,"outputTokens":7,"cacheReadInputTokens":2}}"#,
    );
    let invoke_chunk = base64::engine::general_purpose::STANDARD.encode(
        br#"{"type":"message_stop","amazon-bedrock-invocationMetrics":{"inputTokenCount":30,"outputTokenCount":9}}"#,
    );
    let invoke = message(format!(r#"{{"bytes":"{invoke_chunk}"}}"#).as_bytes());
    let mut body = message(br#"{"contentBlockDelta":{"delta":{"text":"hi"}}}"#);
    body.extend(converse);
    body.extend(invoke);
    let parsed = inference::audit::eventstream_usage_for_test(&body);
    assert_eq!(parsed, (Some(30), Some(9), Some(2)));
    assert_eq!(
        inference::audit::seconds_for_test(
            br#"{"text":"hi","usage":{"type":"duration","seconds":12.2}}"#
        ),
        Some(13)
    );
    assert_eq!(
        inference::audit::seconds_for_test(br#"{"text":"hi","duration":4.0}"#),
        Some(4)
    );
}

#[test]
fn sidecar_usage_frames_are_bounded() {
    let good = record(
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        "gpt-5-nano",
        10,
        5,
        None,
    )
    .wire();
    assert!(Record::from_wire(Uuid::new_v4(), good.clone()).is_ok());
    for (name, frame) in [
        ("negative tokens", {
            let mut w = good.clone();
            w.input_tokens = Some(-1);
            w
        }),
        ("absurd tokens", {
            let mut w = good.clone();
            w.output_tokens = Some(1_000_000_000_000);
            w
        }),
        ("bad status", {
            let mut w = good.clone();
            w.status = 1000;
            w
        }),
        ("negative bytes", {
            let mut w = good.clone();
            w.request_bytes = -5;
            w
        }),
        ("unknown provider", {
            let mut w = good.clone();
            w.provider_id = "mystery".into();
            w
        }),
        ("not a uuid", {
            let mut w = good.clone();
            w.connection_id = "nope".into();
            w
        }),
    ] {
        assert!(Record::from_wire(Uuid::new_v4(), frame).is_err(), "{name}");
    }
}

#[tokio::test]
async fn prices_cost_requests_and_budgets_gate_the_inference_claim() {
    let db = Database::new().await;
    let encryption = Arc::new(Encryption::initialize(&db.pool, seed(21)).await.unwrap());
    let agents = Agents::new(db.pool.clone(), encryption.clone());
    let connections = Connections::new(
        db.pool.clone(),
        encryption.clone(),
        "http://127.0.0.1:1".into(),
        "http://127.0.0.1:2".into(),
    )
    .unwrap();
    connections.seed().await.unwrap();
    // The sheet lands, is idempotent, and leaves manual rows alone.
    let count = inference::prices::reconcile(&db.pool).await.unwrap();
    assert!(count > 500, "{count}");
    assert_eq!(inference::prices::reconcile(&db.pool).await.unwrap(), count);
    let nano = inference::db::price_get_opt(&db.pool.get().await.unwrap(), "openai", "gpt-5-nano")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            nano.input_per_m_micros,
            nano.output_per_m_micros,
            nano.cached_input_per_m_micros
        ),
        (Some(50_000), Some(400_000), Some(5_000))
    );
    db.pool.get().await.unwrap().execute("INSERT INTO inference_prices(provider_id,model,input_per_m_micros,output_per_m_micros,cached_input_per_m_micros,source) VALUES('openai','gpt-5-nano',1,2,3,'manual') ON CONFLICT (provider_id,model) DO UPDATE SET input_per_m_micros=1,output_per_m_micros=2,cached_input_per_m_micros=3,source='manual'", &[]).await.unwrap();
    inference::prices::reconcile(&db.pool).await.unwrap();
    let manual =
        inference::db::price_get_opt(&db.pool.get().await.unwrap(), "openai", "gpt-5-nano")
            .await
            .unwrap()
            .unwrap();
    assert_eq!(
        (manual.input_per_m_micros, manual.source.as_str()),
        (Some(1), "manual")
    );
    db.pool.get().await.unwrap().execute("UPDATE inference_prices SET source='litellm' WHERE provider_id='openai' AND model='gpt-5-nano'", &[]).await.unwrap();
    inference::prices::reconcile(&db.pool).await.unwrap();

    let agent = Uuid::new_v4();
    agents
        .create(CreateAgent {
            concurrency_policy: Default::default(),
            id: agent,
            name: "Budget".into(),
            capabilities: Default::default(),
        })
        .await
        .unwrap();
    // A ready inference connection assigned to the agent, seeded as the broker would leave it.
    let connection = Uuid::new_v4();
    connections
        .start(
            connection,
            "prod",
            "openai",
            "api",
            &[Assignment {
                capability: Capability::Inference,
                agent_id: agent,
                alias: None,
            }],
        )
        .await
        .unwrap();
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE connections SET status='ready' WHERE id=$1",
            &[&connection],
        )
        .await
        .unwrap();
    let chat = Chat::new(
        db.pool.clone(),
        encryption.clone(),
        "http://127.0.0.1:1".into(),
    );
    let thread = chat
        .create_thread(application::CreateThread {
            title: "Budget".into(),
            primary_agent_id: agent.to_string(),
            participants: vec![types::ParticipantRef {
                agent_id: Some(agent.to_string()),
                ..Default::default()
            }],
        })
        .await
        .unwrap();
    let run = chat
        .start_run(application::StartRun {
            thread_id: thread.id.clone(),
            agent_id: agent.to_string(),
            objective: "spend".into(),
            idempotency_key: Uuid::new_v4().to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    let invocation = Uuid::parse_str(&run.invocation_id).unwrap();
    let (thread, run) = (
        Uuid::parse_str(&thread.id).unwrap(),
        Uuid::parse_str(&run.id).unwrap(),
    );
    db.pool.get().await.unwrap().execute("UPDATE chat_invocations SET status='running',lease_expires_at=NOW()+INTERVAL '5 minutes' WHERE id=$1", &[&invocation]).await.unwrap();
    let claim = |token: &SecretString| {
        let chat = chat.clone();
        let token = token.expose_secret().to_owned();
        async move { chat.tokens.scope(&token).await.unwrap().inference }
    };
    let token = chat
        .tokens
        .issue(agent, invocation, thread, run)
        .await
        .unwrap();
    assert_eq!(claim(&token).await, vec![connection]);

    // Costs are priced at insert: 1M input (200k cached) + 10k output on gpt-5-nano.
    // (800k * 0.05 + 200k * 0.005 + 10k * 0.40) per million = 40_000 + 1_000 + 4_000 micro-dollars.
    let unknown = record(agent, connection, run, "gpt-unknown", 1000, 10, None);
    let priced = record(
        agent,
        connection,
        run,
        "gpt-5-nano",
        1_000_000,
        10_000,
        Some(200_000),
    );
    inference::db::requests_insert_execute(
        &db.pool.get().await.unwrap(),
        &[priced.clone(), unknown.clone()],
    )
    .await
    .unwrap();
    let rows = inference::db::requests_for_agent_all(&db.pool.get().await.unwrap(), agent)
        .await
        .unwrap();
    let cost = |id: Uuid| rows.iter().find(|r| r.id == id).unwrap().cost_micros;
    assert_eq!(cost(priced.id), Some(45_000));
    assert_eq!(cost(unknown.id), None);
    // Anthropic reports cache reads and writes beside input; writes bill at a premium.
    // 1000 input * 3 + 1000 cached * 0.3 + 1000 written * 3.75 + 100 output * 15 = 8_550 micro-dollars.
    let mut sonnet = record(
        agent,
        connection,
        run,
        "claude-sonnet-4-5",
        1_000,
        100,
        Some(1_000),
    );
    sonnet.provider_id = "anthropic".into();
    sonnet.cache_write_tokens = Some(1_000);
    inference::db::requests_insert_execute(&db.pool.get().await.unwrap(), &[sonnet.clone()])
        .await
        .unwrap();
    let rows = inference::db::requests_for_agent_all(&db.pool.get().await.unwrap(), agent)
        .await
        .unwrap();
    assert_eq!(
        rows.iter().find(|r| r.id == sonnet.id).unwrap().cost_micros,
        Some(8_550)
    );
    let usage = inference::db::usage_by_connection_all(
        &db.pool.get().await.unwrap(),
        agent,
        chrono::Utc::now() - chrono::Duration::hours(1),
        chrono::Utc::now() + chrono::Duration::hours(1),
    )
    .await
    .unwrap();
    assert_eq!(
        (
            usage[0].connection_id,
            usage[0].requests,
            usage[0].cost_micros,
            usage[0].unpriced_requests
        ),
        (connection, 3, Some(53_550), 1),
        "the unknown-model call is counted but unpriced"
    );

    // Images bill per image and speech per character, from the request rather than the response.
    db.pool.get().await.unwrap().execute("INSERT INTO inference_prices(provider_id,model,image_micros,character_per_m_micros,source) VALUES('openai','gpt-image-test',40000,15000000,'manual'),('openai','tts-test',NULL,15000000,'manual')", &[]).await.unwrap();
    let mut image = record(agent, connection, run, "gpt-image-test", 0, 0, None);
    image.kind = "image".into();
    image.units = Some(3);
    let mut speech = record(agent, connection, run, "tts-test", 0, 0, None);
    speech.kind = "speech".into();
    speech.units = Some(2_000);
    inference::db::requests_insert_execute(
        &db.pool.get().await.unwrap(),
        &[image.clone(), speech.clone()],
    )
    .await
    .unwrap();
    let rows = inference::db::requests_for_agent_all(&db.pool.get().await.unwrap(), agent)
        .await
        .unwrap();
    assert_eq!(
        rows.iter().find(|r| r.id == image.id).unwrap().cost_micros,
        Some(120_000)
    );
    assert_eq!(
        rows.iter().find(|r| r.id == speech.id).unwrap().cost_micros,
        Some(30_000)
    );
    // Transcription bills per audio second, read from the response.
    db.pool.get().await.unwrap().execute("INSERT INTO inference_prices(provider_id,model,second_micros,source) VALUES('openai','whisper-test',100,'manual')", &[]).await.unwrap();
    let mut transcript = record(agent, connection, run, "whisper-test", 0, 0, None);
    transcript.kind = "transcription".into();
    transcript.units = Some(95);
    inference::db::requests_insert_execute(&db.pool.get().await.unwrap(), &[transcript.clone()])
        .await
        .unwrap();
    let rows = inference::db::requests_for_agent_all(&db.pool.get().await.unwrap(), agent)
        .await
        .unwrap();
    assert_eq!(
        rows.iter()
            .find(|r| r.id == transcript.id)
            .unwrap()
            .cost_micros,
        Some(9_500)
    );
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "DELETE FROM inference_requests WHERE id=$1",
            &[&transcript.id],
        )
        .await
        .unwrap();
    // A token-priced image model (gpt-image-1 style) falls back to the tokens its response reported.
    db.pool.get().await.unwrap().execute("INSERT INTO inference_prices(provider_id,model,input_per_m_micros,output_per_m_micros,source) VALUES('openai','gpt-image-tokens',5000000,40000000,'manual')", &[]).await.unwrap();
    let mut token_image = record(agent, connection, run, "gpt-image-tokens", 100, 1_000, None);
    token_image.kind = "image".into();
    token_image.units = Some(1);
    inference::db::requests_insert_execute(&db.pool.get().await.unwrap(), &[token_image.clone()])
        .await
        .unwrap();
    let rows = inference::db::requests_for_agent_all(&db.pool.get().await.unwrap(), agent)
        .await
        .unwrap();
    assert_eq!(
        rows.iter()
            .find(|r| r.id == token_image.id)
            .unwrap()
            .cost_micros,
        Some(40_500)
    );
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "DELETE FROM inference_requests WHERE id=$1",
            &[&token_image.id],
        )
        .await
        .unwrap();
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "DELETE FROM inference_requests WHERE id = ANY($1)",
            &[&vec![image.id, speech.id]],
        )
        .await
        .unwrap();
    // A flag budget records spend without blocking; a block budget below spend withholds the claim.
    let flag = budgets::set(
        &db.pool,
        Scope::Agent,
        agent,
        None,
        Period::Month,
        1_000,
        Action::Flag,
    )
    .await
    .unwrap();
    assert_eq!(
        (flag.spent_micros, flag.exhausted_until.is_none()),
        (53_550, true)
    );
    assert_eq!(
        claim(&chat.tokens.renew(token.expose_secret()).await.unwrap()).await,
        vec![connection]
    );
    let block = budgets::set(
        &db.pool,
        Scope::Agent,
        agent,
        None,
        Period::Day,
        40_000,
        Action::Block,
    )
    .await
    .unwrap();
    assert!(block.exhausted_until.is_some());
    let renewed = chat.tokens.renew(token.expose_secret()).await.unwrap();
    assert!(
        claim(&renewed).await.is_empty(),
        "blocked agents get no inference connections"
    );
    assert!(
        claim(
            &chat
                .tokens
                .issue(agent, invocation, thread, run)
                .await
                .unwrap()
        )
        .await
        .is_empty(),
        "fresh tokens are blocked too"
    );
    let blocked = budgets::blocked(&db.pool).await.unwrap();
    assert_eq!(blocked.agents, vec![agent]);
    // Raising the limit clears exhaustion at once; the next renewal carries the connection again.
    budgets::set(
        &db.pool,
        Scope::Agent,
        agent,
        None,
        Period::Day,
        1_000_000,
        Action::Block,
    )
    .await
    .unwrap();
    assert_eq!(
        claim(&chat.tokens.renew(renewed.expose_secret()).await.unwrap()).await,
        vec![connection]
    );
    // A per-connection budget withholds only that connection.
    let other = Uuid::new_v4();
    connections
        .start(
            other,
            "backup",
            "openai",
            "api",
            &[Assignment {
                capability: Capability::Inference,
                agent_id: agent,
                alias: None,
            }],
        )
        .await
        .unwrap();
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE connections SET status='ready' WHERE id=$1",
            &[&other],
        )
        .await
        .unwrap();
    let capped = budgets::set(
        &db.pool,
        Scope::Agent,
        agent,
        Some(connection),
        Period::Month,
        1_000,
        Action::Block,
    )
    .await
    .unwrap();
    assert_eq!(
        (
            capped.connection_id,
            capped.spent_micros,
            capped.exhausted_until.is_some()
        ),
        (Some(connection), 53_550, true)
    );
    assert_eq!(
        claim(&chat.tokens.renew(token.expose_secret()).await.unwrap()).await,
        vec![other]
    );
    assert_eq!(
        budgets::blocked(&db.pool).await.unwrap().assignments,
        vec![(agent, connection)]
    );
    assert!(
        budgets::set(
            &db.pool,
            Scope::Identity,
            agent,
            Some(connection),
            Period::Month,
            1,
            Action::Block
        )
        .await
        .is_err()
    );
    budgets::delete(&db.pool, capped.id).await.unwrap();
    connections
        .unassign(
            other,
            &Assignment {
                capability: Capability::Inference,
                agent_id: agent,
                alias: None,
            },
        )
        .await
        .unwrap();
    let series = inference::db::usage_series_all(
        &db.pool.get().await.unwrap(),
        agent,
        chrono::Utc::now() - chrono::Duration::hours(1),
        chrono::Utc::now() + chrono::Duration::hours(1),
        "day",
    )
    .await
    .unwrap();
    assert_eq!(
        (series.len(), series[0].connection_id, series[0].cost_micros),
        (1, connection, Some(53_550))
    );
    // An identity budget follows the run's source identity, not the agent.
    let identity = Uuid::new_v4();
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "INSERT INTO chat_users(id,name) VALUES($1,'Alice')",
            &[&identity],
        )
        .await
        .unwrap();
    db.pool.get().await.unwrap().execute("INSERT INTO chat_channel_identities(id,connection_id,identity_type,value) VALUES($1,$2,'email','alice@example.com')", &[&identity, &connection]).await.unwrap();
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE chat_runs SET source_identity_id=$1 WHERE id=$2",
            &[&identity, &run],
        )
        .await
        .unwrap();
    // Identity budgets target a root identity; the run's sender rolls up to it.
    assert!(
        budgets::set(
            &db.pool,
            Scope::Identity,
            identity,
            None,
            Period::Total,
            10,
            Action::Block
        )
        .await
        .is_err(),
        "an unrooted identity is not a budget target"
    );
    let root = Uuid::new_v4();
    db.pool
        .get()
        .await
        .unwrap()
        .execute("INSERT INTO root_identities(id) VALUES($1)", &[&root])
        .await
        .unwrap();
    db.pool
        .get()
        .await
        .unwrap()
        .execute(
            "UPDATE chat_users SET root_identity_id=$1 WHERE id=$2",
            &[&root, &identity],
        )
        .await
        .unwrap();
    budgets::set(
        &db.pool,
        Scope::Identity,
        root,
        None,
        Period::Total,
        10,
        Action::Block,
    )
    .await
    .unwrap();
    assert!(
        claim(&chat.tokens.renew(token.expose_secret()).await.unwrap())
            .await
            .is_empty(),
        "identity over budget"
    );
    assert_eq!(
        budgets::blocked(&db.pool).await.unwrap().identities,
        vec![identity],
        "expanded to the member identity"
    );
    let listed = budgets::list(&db.pool, Some(Scope::Identity), Some(root))
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    budgets::delete(&db.pool, listed[0].id).await.unwrap();
    assert!(budgets::delete(&db.pool, listed[0].id).await.is_err());
    assert_eq!(
        claim(&chat.tokens.renew(token.expose_secret()).await.unwrap()).await,
        vec![connection]
    );
    db.close().await;
}
