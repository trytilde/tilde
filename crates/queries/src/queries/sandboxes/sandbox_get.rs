// This file was generated with `cornucopia`. Do not modify.

#[derive(Clone, Copy, Debug)]
pub struct RunParams {
    pub p2: f64,
    pub p3: chrono::DateTime<chrono::Utc>,
    pub p1: uuid::Uuid,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub blueprint_id: uuid::Uuid,
    pub reuse: String,
    pub connection_id: uuid::Uuid,
    pub agent_id: Option<uuid::Uuid>,
    pub thread_id: Option<uuid::Uuid>,
    pub identity_id: Option<uuid::Uuid>,
    pub status: String,
    pub provider_sandbox_id: Option<String>,
    pub snapshot_id: Option<String>,
    pub error: String,
    pub live: bool,
    pub registered: bool,
    pub leased: bool,
    pub template: String,
    pub connect_timeout_secs: i32,
    pub terminate_after_secs: i32,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub blueprint_id: uuid::Uuid,
    pub reuse: &'a str,
    pub connection_id: uuid::Uuid,
    pub agent_id: Option<uuid::Uuid>,
    pub thread_id: Option<uuid::Uuid>,
    pub identity_id: Option<uuid::Uuid>,
    pub status: &'a str,
    pub provider_sandbox_id: Option<&'a str>,
    pub snapshot_id: Option<&'a str>,
    pub error: &'a str,
    pub live: bool,
    pub registered: bool,
    pub leased: bool,
    pub template: &'a str,
    pub connect_timeout_secs: i32,
    pub terminate_after_secs: i32,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            blueprint_id,
            reuse,
            connection_id,
            agent_id,
            thread_id,
            identity_id,
            status,
            provider_sandbox_id,
            snapshot_id,
            error,
            live,
            registered,
            leased,
            template,
            connect_timeout_secs,
            terminate_after_secs,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            blueprint_id,
            reuse: reuse.into(),
            connection_id,
            agent_id,
            thread_id,
            identity_id,
            status: status.into(),
            provider_sandbox_id: provider_sandbox_id.map(|v| v.into()),
            snapshot_id: snapshot_id.map(|v| v.into()),
            error: error.into(),
            live,
            registered,
            leased,
            template: template.into(),
            connect_timeout_secs,
            terminate_after_secs,
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
        "SELECT s.id,s.blueprint_id,s.reuse,s.connection_id,s.agent_id,s.thread_id,s.identity_id,s.status,s.provider_sandbox_id,s.snapshot_id,s.error, COALESCE(s.connected_at > NOW() - make_interval(secs => $1), false) AS live, COALESCE(s.connected_at > $2, false) AS registered, COALESCE(s.lease_until > NOW(), false) AS leased, b.template,b.connect_timeout_secs,b.terminate_after_secs FROM sandboxes s JOIN sandbox_blueprints b ON b.id=s.blueprint_id WHERE s.id=$3",
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
        p2: &'a f64,
        p3: &'a chrono::DateTime<chrono::Utc>,
        p1: &'a uuid::Uuid,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 3> {
        RecordQuery {
            client,
            params: [p2, p3, p1],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        id: row.try_get(0)?,
                        blueprint_id: row.try_get(1)?,
                        reuse: row.try_get(2)?,
                        connection_id: row.try_get(3)?,
                        agent_id: row.try_get(4)?,
                        thread_id: row.try_get(5)?,
                        identity_id: row.try_get(6)?,
                        status: row.try_get(7)?,
                        provider_sandbox_id: row.try_get(8)?,
                        snapshot_id: row.try_get(9)?,
                        error: row.try_get(10)?,
                        live: row.try_get(11)?,
                        registered: row.try_get(12)?,
                        leased: row.try_get(13)?,
                        template: row.try_get(14)?,
                        connect_timeout_secs: row.try_get(15)?,
                        terminate_after_secs: row.try_get(16)?,
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
        self.bind(client, &params.p2, &params.p3, &params.p1)
    }
}
