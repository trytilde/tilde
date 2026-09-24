// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub connection_id: uuid::Uuid,
    pub invocation_id: uuid::Uuid,
    pub thread_id: uuid::Uuid,
    pub run_id: uuid::Uuid,
    pub participant_id: uuid::Uuid,
    pub provider_id: String,
    pub kind: String,
    pub path: String,
    pub model: Option<String>,
    pub status: i32,
    pub latency_ms: i32,
    pub first_byte_ms: Option<i32>,
    pub request_bytes: i64,
    pub response_bytes: i64,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cached_input_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub units: Option<i64>,
    pub usage: String,
    pub cost_micros: Option<i64>,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub connection_id: uuid::Uuid,
    pub invocation_id: uuid::Uuid,
    pub thread_id: uuid::Uuid,
    pub run_id: uuid::Uuid,
    pub participant_id: uuid::Uuid,
    pub provider_id: &'a str,
    pub kind: &'a str,
    pub path: &'a str,
    pub model: Option<&'a str>,
    pub status: i32,
    pub latency_ms: i32,
    pub first_byte_ms: Option<i32>,
    pub request_bytes: i64,
    pub response_bytes: i64,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cached_input_tokens: Option<i64>,
    pub cache_write_tokens: Option<i64>,
    pub units: Option<i64>,
    pub usage: &'a str,
    pub cost_micros: Option<i64>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            created_at,
            connection_id,
            invocation_id,
            thread_id,
            run_id,
            participant_id,
            provider_id,
            kind,
            path,
            model,
            status,
            latency_ms,
            first_byte_ms,
            request_bytes,
            response_bytes,
            input_tokens,
            output_tokens,
            cached_input_tokens,
            cache_write_tokens,
            units,
            usage,
            cost_micros,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            created_at,
            connection_id,
            invocation_id,
            thread_id,
            run_id,
            participant_id,
            provider_id: provider_id.into(),
            kind: kind.into(),
            path: path.into(),
            model: model.map(|v| v.into()),
            status,
            latency_ms,
            first_byte_ms,
            request_bytes,
            response_bytes,
            input_tokens,
            output_tokens,
            cached_input_tokens,
            cache_write_tokens,
            units,
            usage: usage.into(),
            cost_micros,
        }
    }
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RecordQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<RecordBorrowed, tokio_postgres::Error>,
    mapper: fn(RecordBorrowed) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> RecordQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(self, mapper: fn(RecordBorrowed) -> R) -> RecordQuery<'c, 'a, 's, C, R, N> {
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
        "SELECT id,created_at,connection_id,invocation_id,thread_id,run_id,participant_id,provider_id,kind,path,model,status,latency_ms,first_byte_ms,request_bytes,response_bytes,input_tokens,output_tokens,cached_input_tokens,cache_write_tokens,units,usage,cost_micros FROM inference_requests WHERE agent_id=$1 ORDER BY created_at DESC,id LIMIT 200",
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
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        id: row.try_get(0)?,
                        created_at: row.try_get(1)?,
                        connection_id: row.try_get(2)?,
                        invocation_id: row.try_get(3)?,
                        thread_id: row.try_get(4)?,
                        run_id: row.try_get(5)?,
                        participant_id: row.try_get(6)?,
                        provider_id: row.try_get(7)?,
                        kind: row.try_get(8)?,
                        path: row.try_get(9)?,
                        model: row.try_get(10)?,
                        status: row.try_get(11)?,
                        latency_ms: row.try_get(12)?,
                        first_byte_ms: row.try_get(13)?,
                        request_bytes: row.try_get(14)?,
                        response_bytes: row.try_get(15)?,
                        input_tokens: row.try_get(16)?,
                        output_tokens: row.try_get(17)?,
                        cached_input_tokens: row.try_get(18)?,
                        cache_write_tokens: row.try_get(19)?,
                        units: row.try_get(20)?,
                        usage: row.try_get(21)?,
                        cost_micros: row.try_get(22)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
