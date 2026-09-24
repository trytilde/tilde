// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::ArraySql<Item = uuid::Uuid>> {
    pub p1: T1,
    pub p2: chrono::DateTime<chrono::Utc>,
}
#[derive(Debug, Clone, PartialEq, Copy)]
pub struct Record {
    pub id: uuid::Uuid,
    pub thread_count: i64,
    pub average_turns_per_thread: Option<f64>,
    pub average_response_ms: Option<f64>,
    pub healthy: Option<bool>,
    pub degraded: Option<bool>,
    pub checked_at: Option<chrono::DateTime<chrono::Utc>>,
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
        "WITH selected AS (SELECT a.id FROM agents a WHERE a.deleted_at IS NULL AND a.id=ANY($1)), sessions AS ( SELECT p.agent_id, COUNT(*) AS count FROM chat_participants p JOIN selected a ON a.id = p.agent_id GROUP BY p.agent_id ), turns AS ( SELECT p.agent_id, COUNT(*) AS count FROM chat_messages m JOIN chat_participants p ON p.id = m.participant_id JOIN selected a ON a.id = p.agent_id WHERE m.status = 'complete' AND length(m.text) > 0 GROUP BY p.agent_id ), first_replies AS ( SELECT i.id, i.agent_id, i.started_at, MIN(m.first_content_at) AS first_content_at FROM chat_invocations i JOIN selected a ON a.id = i.agent_id JOIN chat_messages m ON m.invocation_id = i.id JOIN chat_participants p ON p.id = m.participant_id AND p.agent_id = i.agent_id WHERE i.started_at IS NOT NULL AND m.first_content_at >= i.started_at GROUP BY i.id, i.agent_id, i.started_at ), responses AS ( SELECT agent_id, AVG(EXTRACT(EPOCH FROM (first_content_at - started_at)) * 1000)::DOUBLE PRECISION AS avg_ms FROM first_replies GROUP BY agent_id ) SELECT a.id, COALESCE(s.count, 0)::BIGINT AS thread_count, (COALESCE(t.count, 0)::DOUBLE PRECISION / NULLIF(s.count, 0)) AS average_turns_per_thread, r.avg_ms AS average_response_ms, h.healthy, h.degraded, h.checked_at FROM selected a LEFT JOIN sessions s ON s.agent_id = a.id LEFT JOIN turns t ON t.agent_id = a.id LEFT JOIN responses r ON r.agent_id = a.id LEFT JOIN LATERAL ( SELECT healthy, degraded, checked_at FROM agent_health WHERE agent_id = a.id AND sidecar_event_id IS NULL AND checked_at <= $2 ORDER BY checked_at DESC, id DESC LIMIT 1 ) h ON TRUE",
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
    pub fn bind<'c, 'a, 's, C: GenericClient, T1: crate::ArraySql<Item = uuid::Uuid>>(
        &'s self,
        client: &'c C,
        p1: &'a T1,
        p2: &'a chrono::DateTime<chrono::Utc>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        RecordQuery {
            client,
            params: [p1, p2],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row: &tokio_postgres::Row| -> Result<Record, tokio_postgres::Error> {
                Ok(Record {
                    id: row.try_get(0)?,
                    thread_count: row.try_get(1)?,
                    average_turns_per_thread: row.try_get(2)?,
                    average_response_ms: row.try_get(3)?,
                    healthy: row.try_get(4)?,
                    degraded: row.try_get(5)?,
                    checked_at: row.try_get(6)?,
                })
            },
            mapper: |it| Record::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::ArraySql<Item = uuid::Uuid>>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        RunParams<T1>,
        RecordQuery<'c, 'a, 's, C, Record, 2>,
        C,
    > for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        self.bind(client, &params.p1, &params.p2)
    }
}
