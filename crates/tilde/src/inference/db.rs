//! Typed PostgreSQL operations generated from the SQL contracts. Callers own transactions.
#![allow(clippy::too_many_arguments)]
use crate::database::{DbResult, GenericClient};

pub use tilde_queries::queries::inference::requests_for_agent::Record as RequestRow;
pub use tilde_queries::queries::inference::upstreams::Record as UpstreamRow;

pub use tilde_queries::queries::inference::aliases::Record as AliasRow;

pub async fn aliases_all(db: &impl GenericClient) -> DbResult<Vec<AliasRow>> {
    Ok(tilde_queries::queries::inference::aliases::run()
        .bind(db)
        .all()
        .await?)
}

pub async fn upstreams_all(db: &impl GenericClient) -> DbResult<Vec<UpstreamRow>> {
    Ok(tilde_queries::queries::inference::upstreams::run()
        .bind(db)
        .all()
        .await?)
}

pub async fn requests_for_agent_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Vec<RequestRow>> {
    Ok(tilde_queries::queries::inference::requests_for_agent::run()
        .bind(db, &p1)
        .all()
        .await?)
}

/// One array-unnest insert per batch. Nullable columns travel as sentinels (`''`, `-1`) that
/// the statement turns back into NULL, because array parameters cannot hold NULL elements.
pub async fn requests_insert_execute(
    db: &impl GenericClient,
    records: &[&super::audit::Record],
) -> DbResult<u64> {
    let ids: Vec<_> = records.iter().map(|r| r.id).collect();
    let created: Vec<_> = records.iter().map(|r| r.created_at).collect();
    let agents: Vec<_> = records.iter().map(|r| r.agent_id).collect();
    let connections: Vec<_> = records.iter().map(|r| r.connection_id).collect();
    let invocations: Vec<_> = records.iter().map(|r| r.invocation_id).collect();
    let threads: Vec<_> = records.iter().map(|r| r.thread_id).collect();
    let runs: Vec<_> = records.iter().map(|r| r.run_id).collect();
    let participants: Vec<_> = records.iter().map(|r| r.participant_id).collect();
    let providers: Vec<&str> = records.iter().map(|r| r.provider_id.as_str()).collect();
    let kinds: Vec<&str> = records.iter().map(|r| r.kind.as_str()).collect();
    let paths: Vec<&str> = records.iter().map(|r| r.path.as_str()).collect();
    let models: Vec<&str> = records
        .iter()
        .map(|r| r.model.as_deref().unwrap_or(""))
        .collect();
    let statuses: Vec<i32> = records.iter().map(|r| r.status).collect();
    let latencies: Vec<i32> = records.iter().map(|r| r.latency_ms).collect();
    let first_bytes: Vec<i32> = records
        .iter()
        .map(|r| r.first_byte_ms.unwrap_or(-1))
        .collect();
    let request_bytes: Vec<i64> = records.iter().map(|r| r.request_bytes).collect();
    let response_bytes: Vec<i64> = records.iter().map(|r| r.response_bytes).collect();
    let inputs: Vec<i64> = records
        .iter()
        .map(|r| r.input_tokens.unwrap_or(-1))
        .collect();
    let outputs: Vec<i64> = records
        .iter()
        .map(|r| r.output_tokens.unwrap_or(-1))
        .collect();
    let cached: Vec<i64> = records
        .iter()
        .map(|r| r.cached_input_tokens.unwrap_or(-1))
        .collect();
    let cache_writes: Vec<i64> = records
        .iter()
        .map(|r| r.cache_write_tokens.unwrap_or(-1))
        .collect();
    let units: Vec<i64> = records.iter().map(|r| r.units.unwrap_or(-1)).collect();
    let usages: Vec<&str> = records.iter().map(|r| r.usage.as_str()).collect();
    // Price each distinct model once; a model the sheet does not list costs nothing known.
    let mut rates = std::collections::HashMap::new();
    let mut costs = Vec::with_capacity(records.len());
    for record in records {
        let key = (
            record.provider_id.clone(),
            record.model.clone().unwrap_or_default(),
        );
        if !rates.contains_key(&key) {
            let rate = crate::pricing::rate(db, Some(&key.0), &key.1).await?;
            rates.insert(key.clone(), rate);
        }
        let cost = rates[&key].as_ref().and_then(|rate| {
            rate.cost_micros(crate::pricing::Usage {
                input_tokens: record.input_tokens,
                output_tokens: record.output_tokens,
                cached_input_tokens: record.cached_input_tokens,
                cache_write_tokens: record.cache_write_tokens,
                images: (record.kind == "image").then_some(record.units).flatten(),
                characters: (record.kind == "speech").then_some(record.units).flatten(),
                seconds: (record.kind == "transcription")
                    .then_some(record.units)
                    .flatten(),
            })
        });
        costs.push(cost.unwrap_or(-1));
    }
    Ok(tilde_queries::queries::inference::requests_insert::run()
        .bind(
            db,
            &ids,
            &created,
            &agents,
            &connections,
            &invocations,
            &threads,
            &runs,
            &participants,
            &providers,
            &kinds,
            &paths,
            &models,
            &statuses,
            &latencies,
            &first_bytes,
            &request_bytes,
            &response_bytes,
            &inputs,
            &outputs,
            &cached,
            &cache_writes,
            &units,
            &usages,
            &costs,
        )
        .await?)
}

pub use tilde_queries::queries::inference::budgets_list::Record as BudgetRow;
pub use tilde_queries::queries::inference::budgets_settle::Record as SettledRow;
pub use tilde_queries::queries::inference::exhausted::Record as ExhaustedRow;
pub use tilde_queries::queries::inference::price_get::Record as PriceRow;
pub async fn price_find_opt(db: &impl GenericClient, model: &str) -> DbResult<Option<PriceRow>> {
    Ok(tilde_queries::queries::inference::price_find::run()
        .bind(db, &model)
        .opt()
        .await?
        .map(|r| PriceRow {
            provider_id: r.provider_id,
            model: r.model,
            input_per_m_micros: r.input_per_m_micros,
            output_per_m_micros: r.output_per_m_micros,
            cached_input_per_m_micros: r.cached_input_per_m_micros,
            cache_write_per_m_micros: r.cache_write_per_m_micros,
            image_micros: r.image_micros,
            character_per_m_micros: r.character_per_m_micros,
            second_micros: r.second_micros,
            source: r.source,
        }))
}
pub use tilde_queries::queries::inference::usage_by_connection::Record as UsageRow;

pub async fn prices_upsert_execute(
    db: &impl GenericClient,
    providers: &[&str],
    models: &[&str],
    input: &[i64],
    output: &[i64],
    cached: &[i64],
    cache_write: &[i64],
    image: &[i64],
    character: &[i64],
    second: &[i64],
) -> DbResult<u64> {
    Ok(tilde_queries::queries::inference::prices_upsert::run()
        .bind(
            db,
            &providers,
            &models,
            &input,
            &output,
            &cached,
            &cache_write,
            &image,
            &character,
            &second,
        )
        .await?)
}
pub async fn price_get_opt(
    db: &impl GenericClient,
    p1: &str,
    p2: &str,
) -> DbResult<Option<PriceRow>> {
    Ok(tilde_queries::queries::inference::price_get::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?)
}
/// Connections an agent may bill inference to; one read per publish batch.
pub async fn assigned_connections_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Vec<uuid::Uuid>> {
    Ok(
        tilde_queries::queries::inference::assigned_connections::run()
            .bind(db, &p1)
            .all()
            .await?
            .into_iter()
            .map(|r| r.connection_id)
            .collect(),
    )
}
pub async fn usage_total_one(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<i64> {
    Ok(tilde_queries::queries::inference::usage_total::run()
        .bind(db, &p1)
        .one()
        .await?
        .cost_micros)
}
pub async fn root_identity_exists_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Option<tilde_queries::queries::inference::root_identity_exists::Record>> {
    Ok(
        tilde_queries::queries::inference::root_identity_exists::run()
            .bind(db, &p1)
            .opt()
            .await?,
    )
}
pub async fn usage_by_connection_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: chrono::DateTime<chrono::Utc>,
    p3: chrono::DateTime<chrono::Utc>,
) -> DbResult<Vec<UsageRow>> {
    Ok(
        tilde_queries::queries::inference::usage_by_connection::run()
            .bind(db, &p1, &p2, &p3)
            .all()
            .await?,
    )
}
pub use tilde_queries::queries::inference::usage_series::Record as UsagePointRow;

pub async fn usage_series_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: chrono::DateTime<chrono::Utc>,
    p3: chrono::DateTime<chrono::Utc>,
    p4: &str,
) -> DbResult<Vec<UsagePointRow>> {
    // Parameter order follows first appearance in the statement: the bucket comes first.
    Ok(tilde_queries::queries::inference::usage_series::run()
        .bind(db, &p4, &p1, &p2, &p3)
        .all()
        .await?)
}
pub async fn budgets_list_all(
    db: &impl GenericClient,
    p1: Option<&str>,
    p2: Option<uuid::Uuid>,
) -> DbResult<Vec<BudgetRow>> {
    Ok(tilde_queries::queries::inference::budgets_list::run()
        .bind(db, &p1, &p2)
        .all()
        .await?)
}
pub async fn budget_set_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &str,
    p3: uuid::Uuid,
    p4: Option<uuid::Uuid>,
    p5: &str,
    p6: i64,
    p7: &str,
) -> DbResult<tilde_queries::queries::inference::budget_set::Record> {
    Ok(tilde_queries::queries::inference::budget_set::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5, &p6, &p7)
        .one()
        .await?)
}
pub async fn budget_delete_execute(db: &impl GenericClient, p1: uuid::Uuid) -> DbResult<u64> {
    Ok(tilde_queries::queries::inference::budget_delete::run()
        .bind(db, &p1)
        .await?)
}
pub async fn budgets_settle_all(db: &impl GenericClient) -> DbResult<Vec<SettledRow>> {
    Ok(tilde_queries::queries::inference::budgets_settle::run()
        .bind(db)
        .all()
        .await?)
}
pub async fn exhausted_all(db: &impl GenericClient) -> DbResult<Vec<ExhaustedRow>> {
    Ok(tilde_queries::queries::inference::exhausted::run()
        .bind(db)
        .all()
        .await?)
}
