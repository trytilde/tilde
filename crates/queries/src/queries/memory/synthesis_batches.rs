// This file was generated with `cornucopia`. Do not modify.

#[derive(Clone, Copy, Debug)]
pub struct RunParams {
    pub p1: uuid::Uuid,
    pub p2: i64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub thread_id: Option<uuid::Uuid>,
    pub agent_id: Option<uuid::Uuid>,
    pub page_id: Option<uuid::Uuid>,
    pub revision: Option<i32>,
    pub wiki_id: Option<uuid::Uuid>,
    pub path: Option<String>,
    pub status: String,
    pub written: i32,
    pub superseded: i32,
    pub forgotten: i32,
    pub dropped: i32,
    pub notes: String,
    pub at: chrono::DateTime<chrono::Utc>,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub thread_id: Option<uuid::Uuid>,
    pub agent_id: Option<uuid::Uuid>,
    pub page_id: Option<uuid::Uuid>,
    pub revision: Option<i32>,
    pub wiki_id: Option<uuid::Uuid>,
    pub path: Option<&'a str>,
    pub status: &'a str,
    pub written: i32,
    pub superseded: i32,
    pub forgotten: i32,
    pub dropped: i32,
    pub notes: &'a str,
    pub at: chrono::DateTime<chrono::Utc>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            thread_id,
            agent_id,
            page_id,
            revision,
            wiki_id,
            path,
            status,
            written,
            superseded,
            forgotten,
            dropped,
            notes,
            at,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            thread_id,
            agent_id,
            page_id,
            revision,
            wiki_id,
            path: path.map(|v| v.into()),
            status: status.into(),
            written,
            superseded,
            forgotten,
            dropped,
            notes: notes.into(),
            at,
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
        "SELECT b.id,b.thread_id,t.primary_agent_id AS agent_id,b.page_id,b.revision, COALESCE(pg.wiki_id,r.wiki_id) AS wiki_id,COALESCE(pg.path,r.path) AS path, b.status,b.written,b.superseded,b.forgotten,b.dropped,b.notes,b.at FROM memory_synthesis_batches b LEFT JOIN chat_threads t ON t.id=b.thread_id LEFT JOIN wiki_page_revisions r ON r.page_id=b.page_id AND r.revision=b.revision LEFT JOIN wiki_pages pg ON pg.id=b.page_id WHERE b.bank_id=$1 ORDER BY b.at DESC LIMIT $2",
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
        p2: &'a i64,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        RecordQuery {
            client,
            params: [p1, p2],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        id: row.try_get(0)?,
                        thread_id: row.try_get(1)?,
                        agent_id: row.try_get(2)?,
                        page_id: row.try_get(3)?,
                        revision: row.try_get(4)?,
                        wiki_id: row.try_get(5)?,
                        path: row.try_get(6)?,
                        status: row.try_get(7)?,
                        written: row.try_get(8)?,
                        superseded: row.try_get(9)?,
                        forgotten: row.try_get(10)?,
                        dropped: row.try_get(11)?,
                        notes: row.try_get(12)?,
                        at: row.try_get(13)?,
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
        self.bind(client, &params.p1, &params.p2)
    }
}
