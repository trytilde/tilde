// This file was generated with `cornucopia`. Do not modify.

#[derive(Clone, Copy, Debug)]
pub struct RunParams {
    pub p2: i64,
    pub p3: i64,
    pub p1: uuid::Uuid,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub primary_agent_id: uuid::Uuid,
    pub sequence: Option<i64>,
    pub kind: Option<String>,
    pub entity_id: Option<uuid::Uuid>,
    pub text_delta: Option<String>,
    pub snapshot: Option<Vec<u8>>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}
pub struct RecordBorrowed<'a> {
    pub primary_agent_id: uuid::Uuid,
    pub sequence: Option<i64>,
    pub kind: Option<&'a str>,
    pub entity_id: Option<uuid::Uuid>,
    pub text_delta: Option<&'a str>,
    pub snapshot: Option<&'a [u8]>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            primary_agent_id,
            sequence,
            kind,
            entity_id,
            text_delta,
            snapshot,
            created_at,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            primary_agent_id,
            sequence,
            kind: kind.map(|v| v.into()),
            entity_id,
            text_delta: text_delta.map(|v| v.into()),
            snapshot: snapshot.map(|v| v.into()),
            created_at,
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
        "SELECT t.primary_agent_id, e.sequence AS sequence, e.kind AS kind, e.entity_id AS entity_id, e.text_delta AS text_delta, e.snapshot, e.created_at AS created_at FROM chat_threads t LEFT JOIN LATERAL ( SELECT sequence, kind, entity_id, text_delta, snapshot, created_at FROM chat_activity WHERE thread_id = t.id AND sequence > $1 ORDER BY sequence LIMIT $2 ) e ON TRUE WHERE t.id = $3 ORDER BY e.sequence",
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
        p2: &'a i64,
        p3: &'a i64,
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
                        primary_agent_id: row.try_get(0)?,
                        sequence: row.try_get(1)?,
                        kind: row.try_get(2)?,
                        entity_id: row.try_get(3)?,
                        text_delta: row.try_get(4)?,
                        snapshot: row.try_get(5)?,
                        created_at: row.try_get(6)?,
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
