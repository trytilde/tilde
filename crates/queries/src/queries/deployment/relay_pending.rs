// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq, Copy)]
pub struct Record {
    pub message_id: uuid::Uuid,
    pub thread_id: uuid::Uuid,
    pub agent_id: uuid::Uuid,
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
        "SELECT m.id AS message_id,m.thread_id,p.agent_id AS agent_id FROM chat_messages m JOIN chat_participants p ON p.thread_id=m.thread_id AND p.agent_id IS NOT NULL AND p.active JOIN agents a ON a.id=p.agent_id AND NOT a.paused AND a.deleted_at IS NULL JOIN chat_participants author ON author.id=m.participant_id WHERE NOT EXISTS(SELECT 1 FROM agent_deployments d WHERE d.id=p.deployment_id AND d.target<>'sidecar' AND d.status='registered') AND m.status='complete' AND author.agent_id IS DISTINCT FROM p.agent_id AND NOT EXISTS(SELECT 1 FROM chat_message_dispatch d WHERE d.message_id=m.id AND d.agent_id=p.agent_id) AND EXISTS(SELECT 1 FROM agent_instances n WHERE n.agent_id=p.agent_id AND n.ready AND n.agent_ready AND n.last_seen_at>NOW()-make_interval(secs=>$1::float8)) ORDER BY m.created_at LIMIT 50",
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
        p1: &'a f64,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 1> {
        RecordQuery {
            client,
            params: [p1],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row: &tokio_postgres::Row| -> Result<Record, tokio_postgres::Error> {
                Ok(Record {
                    message_id: row.try_get(0)?,
                    thread_id: row.try_get(1)?,
                    agent_id: row.try_get(2)?,
                })
            },
            mapper: |it| Record::from(it),
        }
    }
}
