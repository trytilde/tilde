// This file was generated with `cornucopia`. Do not modify.

#[derive(Clone, Copy, Debug)]
pub struct RunParams {
    pub agent: Option<uuid::Uuid>,
    pub id: Option<uuid::Uuid>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub agent_id: uuid::Uuid,
    pub name: String,
    pub prompt: String,
    pub enabled: bool,
    pub schedule: Option<String>,
    pub connection_id: Option<uuid::Uuid>,
    pub signal_type: Option<String>,
    pub next_run_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub last_run_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_thread_id: Option<uuid::Uuid>,
    pub last_error: Option<String>,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub agent_id: uuid::Uuid,
    pub name: &'a str,
    pub prompt: &'a str,
    pub enabled: bool,
    pub schedule: Option<&'a str>,
    pub connection_id: Option<uuid::Uuid>,
    pub signal_type: Option<&'a str>,
    pub next_run_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub last_run_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_thread_id: Option<uuid::Uuid>,
    pub last_error: Option<&'a str>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            agent_id,
            name,
            prompt,
            enabled,
            schedule,
            connection_id,
            signal_type,
            next_run_at,
            created_at,
            updated_at,
            last_run_at,
            last_thread_id,
            last_error,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            agent_id,
            name: name.into(),
            prompt: prompt.into(),
            enabled,
            schedule: schedule.map(|v| v.into()),
            connection_id,
            signal_type: signal_type.map(|v| v.into()),
            next_run_at,
            created_at,
            updated_at,
            last_run_at,
            last_thread_id,
            last_error: last_error.map(|v| v.into()),
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
        "SELECT r.id,r.agent_id,r.name,r.prompt,r.enabled,r.schedule,r.connection_id,r.signal_type,r.next_run_at,r.created_at,r.updated_at,last.created_at AS last_run_at,last.thread_id AS last_thread_id,last.error AS last_error FROM routines r LEFT JOIN LATERAL (SELECT created_at,thread_id,error FROM routine_runs WHERE routine_id=r.id ORDER BY created_at DESC LIMIT 1) last ON true WHERE ($1::UUID IS NULL OR r.agent_id=$1) AND ($2::UUID IS NULL OR r.id=$2) ORDER BY r.created_at,r.id",
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
        agent: &'a Option<uuid::Uuid>,
        id: &'a Option<uuid::Uuid>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        RecordQuery {
            client,
            params: [agent, id],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        id: row.try_get(0)?,
                        agent_id: row.try_get(1)?,
                        name: row.try_get(2)?,
                        prompt: row.try_get(3)?,
                        enabled: row.try_get(4)?,
                        schedule: row.try_get(5)?,
                        connection_id: row.try_get(6)?,
                        signal_type: row.try_get(7)?,
                        next_run_at: row.try_get(8)?,
                        created_at: row.try_get(9)?,
                        updated_at: row.try_get(10)?,
                        last_run_at: row.try_get(11)?,
                        last_thread_id: row.try_get(12)?,
                        last_error: row.try_get(13)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient>
    crate::client::async_::Params<'c, 'a, 's, RunParams, RecordQuery<'c, 'a, 's, C, Record, 2>, C>
    for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        self.bind(client, &params.agent, &params.id)
    }
}
