// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub run_id: uuid::Uuid,
    pub thread_id: uuid::Uuid,
    pub agent_id: uuid::Uuid,
    pub traceparent: String,
    pub tracestate: String,
    pub generation: i64,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub run_id: uuid::Uuid,
    pub thread_id: uuid::Uuid,
    pub agent_id: uuid::Uuid,
    pub traceparent: &'a str,
    pub tracestate: &'a str,
    pub generation: i64,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            run_id,
            thread_id,
            agent_id,
            traceparent,
            tracestate,
            generation,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            run_id,
            thread_id,
            agent_id,
            traceparent: traceparent.into(),
            tracestate: tracestate.into(),
            generation,
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
        "WITH eligible AS ( SELECT i.id,a.generation FROM chat_invocations i JOIN agents a ON a.id=i.agent_id JOIN chat_runs r ON r.id=i.run_id WHERE i.id=$1 AND i.status='pending' AND EXISTS(SELECT 1 FROM agent_deployments d WHERE d.id=i.deployment_id AND d.target IN ('gateway','lambda')) AND NOT a.paused AND a.deleted_at IS NULL AND chat_channel_run_allowed(i.agent_id,i.thread_id,r.source_identity_id,r.channel_origin) FOR SHARE OF a ) UPDATE chat_invocations i SET status='running',started_at=NOW(),lease_expires_at=NOW()+INTERVAL '30 seconds' FROM eligible e WHERE i.id=e.id AND i.status='pending' RETURNING i.id,i.run_id,i.thread_id,i.agent_id,i.traceparent,i.tracestate,e.generation AS generation",
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
                        run_id: row.try_get(1)?,
                        thread_id: row.try_get(2)?,
                        agent_id: row.try_get(3)?,
                        traceparent: row.try_get(4)?,
                        tracestate: row.try_get(5)?,
                        generation: row.try_get(6)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
