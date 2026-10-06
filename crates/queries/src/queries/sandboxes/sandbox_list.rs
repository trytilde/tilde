// This file was generated with `cornucopia`. Do not modify.

#[derive(Clone, Copy, Debug)]
pub struct RunParams {
    pub p3: f64,
    pub p1: Option<uuid::Uuid>,
    pub p2: Option<uuid::Uuid>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub blueprint_id: uuid::Uuid,
    pub reuse: String,
    pub agent_id: Option<uuid::Uuid>,
    pub thread_id: Option<uuid::Uuid>,
    pub identity_id: Option<uuid::Uuid>,
    pub status: String,
    pub provider_sandbox_id: Option<String>,
    pub error: String,
    pub last_used_at: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub connected: bool,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub blueprint_id: uuid::Uuid,
    pub reuse: &'a str,
    pub agent_id: Option<uuid::Uuid>,
    pub thread_id: Option<uuid::Uuid>,
    pub identity_id: Option<uuid::Uuid>,
    pub status: &'a str,
    pub provider_sandbox_id: Option<&'a str>,
    pub error: &'a str,
    pub last_used_at: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub connected: bool,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            blueprint_id,
            reuse,
            agent_id,
            thread_id,
            identity_id,
            status,
            provider_sandbox_id,
            error,
            last_used_at,
            created_at,
            connected,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            blueprint_id,
            reuse: reuse.into(),
            agent_id,
            thread_id,
            identity_id,
            status: status.into(),
            provider_sandbox_id: provider_sandbox_id.map(|v| v.into()),
            error: error.into(),
            last_used_at,
            created_at,
            connected,
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
        "SELECT id,blueprint_id,reuse,agent_id,thread_id,identity_id,status,provider_sandbox_id,error,last_used_at,created_at, COALESCE(connected_at > NOW() - make_interval(secs => $1), false) AS connected FROM sandboxes WHERE ($2::UUID IS NULL OR blueprint_id=$2) AND ($3::UUID IS NULL OR agent_id=$3) ORDER BY last_used_at DESC",
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
        p3: &'a f64,
        p1: &'a Option<uuid::Uuid>,
        p2: &'a Option<uuid::Uuid>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 3> {
        RecordQuery {
            client,
            params: [p3, p1, p2],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        id: row.try_get(0)?,
                        blueprint_id: row.try_get(1)?,
                        reuse: row.try_get(2)?,
                        agent_id: row.try_get(3)?,
                        thread_id: row.try_get(4)?,
                        identity_id: row.try_get(5)?,
                        status: row.try_get(6)?,
                        provider_sandbox_id: row.try_get(7)?,
                        error: row.try_get(8)?,
                        last_used_at: row.try_get(9)?,
                        created_at: row.try_get(10)?,
                        connected: row.try_get(11)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient>
    crate::client::async_::Params<'c, 'a, 's, RunParams, RecordQuery<'c, 'a, 's, C, Record, 3>, C>
    for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 3> {
        self.bind(client, &params.p3, &params.p1, &params.p2)
    }
}
