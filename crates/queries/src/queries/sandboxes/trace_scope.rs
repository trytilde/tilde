// This file was generated with `cornucopia`. Do not modify.

#[derive(Clone, Copy, Debug)]
pub struct RunParams {
    pub p2: Option<uuid::Uuid>,
    pub p1: uuid::Uuid,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub invocation_id: uuid::Uuid,
    pub agent_id: uuid::Uuid,
    pub run_id: uuid::Uuid,
    pub thread_id: uuid::Uuid,
    pub traceparent: String,
    pub tracestate: String,
}
pub struct RecordBorrowed<'a> {
    pub invocation_id: uuid::Uuid,
    pub agent_id: uuid::Uuid,
    pub run_id: uuid::Uuid,
    pub thread_id: uuid::Uuid,
    pub traceparent: &'a str,
    pub tracestate: &'a str,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            invocation_id,
            agent_id,
            run_id,
            thread_id,
            traceparent,
            tracestate,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            invocation_id,
            agent_id,
            run_id,
            thread_id,
            traceparent: traceparent.into(),
            tracestate: tracestate.into(),
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
        "SELECT i.id AS invocation_id,i.agent_id,i.run_id,i.thread_id, COALESCE(c.traceparent,'') AS traceparent,COALESCE(c.tracestate,'') AS tracestate FROM sandboxes s LEFT JOIN sandbox_calls c ON c.id=$1 AND c.sandbox_id=s.id JOIN chat_invocations i ON i.id=COALESCE(c.invocation_id,s.invocation_id) WHERE s.id=$2",
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
        p2: &'a Option<uuid::Uuid>,
        p1: &'a uuid::Uuid,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        RecordQuery {
            client,
            params: [p2, p1],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        invocation_id: row.try_get(0)?,
                        agent_id: row.try_get(1)?,
                        run_id: row.try_get(2)?,
                        thread_id: row.try_get(3)?,
                        traceparent: row.try_get(4)?,
                        tracestate: row.try_get(5)?,
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
        self.bind(client, &params.p2, &params.p1)
    }
}
