// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq, Copy)]
pub struct Record {
    pub version_id: uuid::Uuid,
    pub requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_micros: Option<i64>,
    pub average_latency_ms: i64,
    pub last_used_at: chrono::DateTime<chrono::Utc>,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RecordQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<Record, tokio_postgres::Error>,
    mapper: fn(Record) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> RecordQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(self, mapper: fn(Record) -> R) -> RecordQuery<'c, 'a, 's, C, R, N> {
        RecordQuery {
            client: self.client,
            params: self.params,
            query: self.query,
            cached: self.cached,
            extractor: self.extractor,
            mapper,
        }
    }
    pub async fn one(self) -> Result<T, tokio_postgres::Error> {
        let row =
            crate::client::async_::one(self.client, self.query, &self.params, self.cached).await?;
        Ok((self.mapper)((self.extractor)(&row)?))
    }
    pub async fn all(self) -> Result<Vec<T>, tokio_postgres::Error> {
        self.iter().await?.try_collect().await
    }
    pub async fn opt(self) -> Result<Option<T>, tokio_postgres::Error> {
        let opt_row =
            crate::client::async_::opt(self.client, self.query, &self.params, self.cached).await?;
        Ok(opt_row
            .map(|row| {
                let extracted = (self.extractor)(&row)?;
                Ok((self.mapper)(extracted))
            })
            .transpose()?)
    }
    pub async fn iter(
        self,
    ) -> Result<
        impl futures::Stream<Item = Result<T, tokio_postgres::Error>> + 'c,
        tokio_postgres::Error,
    > {
        let stream = crate::client::async_::raw(
            self.client,
            self.query,
            crate::slice_iter(&self.params),
            self.cached,
        )
        .await?;
        let mapped = stream
            .map(move |res| {
                res.and_then(|row| {
                    let extracted = (self.extractor)(&row)?;
                    Ok((self.mapper)(extracted))
                })
            })
            .into_stream();
        Ok(mapped)
    }
}
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "SELECT v.id AS version_id,COUNT(*)::BIGINT AS requests,COALESCE(SUM(r.input_tokens),0)::BIGINT AS input_tokens,COALESCE(SUM(r.output_tokens),0)::BIGINT AS output_tokens, SUM(r.cost_micros)::BIGINT AS cost_micros,COALESCE(AVG(r.latency_ms),0)::BIGINT AS average_latency_ms,MAX(r.created_at) AS last_used_at FROM prompt_versions v JOIN inference_request_prompts l ON l.version_id=v.id JOIN inference_requests r ON r.id=l.request_id WHERE v.prompt_id=$1 GROUP BY v.id",
        None,
    )
}
impl RunStmt {
    pub async fn prepare<'a, C: GenericClient>(
        mut self,
        client: &'a C,
    ) -> Result<Self, tokio_postgres::Error> {
        self.1 = Some(client.prepare(self.0).await?);
        Ok(self)
    }
    pub fn bind<'c, 'a, 's, C: GenericClient>(
        &'s self,
        client: &'c C,
        p1: &'a uuid::Uuid,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 1> {
        RecordQuery {
            client,
            params: [p1],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row: &tokio_postgres::Row| -> Result<Record, tokio_postgres::Error> {
                Ok(Record {
                    version_id: row.try_get(0)?,
                    requests: row.try_get(1)?,
                    input_tokens: row.try_get(2)?,
                    output_tokens: row.try_get(3)?,
                    cost_micros: row.try_get(4)?,
                    average_latency_ms: row.try_get(5)?,
                    last_used_at: row.try_get(6)?,
                })
            },
            mapper: |it| Record::from(it),
        }
    }
}
