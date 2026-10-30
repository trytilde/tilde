// This file was generated with `cornucopia`. Do not modify.

#[derive(Clone, Copy, Debug)]
pub struct RunParams {
    pub p1: uuid::Uuid,
    pub p2: i32,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub page_id: uuid::Uuid,
    pub wiki_id: uuid::Uuid,
    pub revision: i32,
    pub operation: String,
    pub path: String,
    pub title: String,
    pub content: String,
    pub agent_id: Option<uuid::Uuid>,
    pub thread_id: Option<uuid::Uuid>,
    pub at: chrono::DateTime<chrono::Utc>,
}
pub struct RecordBorrowed<'a> {
    pub page_id: uuid::Uuid,
    pub wiki_id: uuid::Uuid,
    pub revision: i32,
    pub operation: &'a str,
    pub path: &'a str,
    pub title: &'a str,
    pub content: &'a str,
    pub agent_id: Option<uuid::Uuid>,
    pub thread_id: Option<uuid::Uuid>,
    pub at: chrono::DateTime<chrono::Utc>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            page_id,
            wiki_id,
            revision,
            operation,
            path,
            title,
            content,
            agent_id,
            thread_id,
            at,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            page_id,
            wiki_id,
            revision,
            operation: operation.into(),
            path: path.into(),
            title: title.into(),
            content: content.into(),
            agent_id,
            thread_id,
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
        "SELECT r.page_id,r.wiki_id,r.revision,r.operation,r.path,r.title,r.content,r.agent_id,r.thread_id,r.at FROM wiki_page_revisions r WHERE r.page_id=$1 AND r.revision=$2",
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
        p2: &'a i32,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        RecordQuery {
            client,
            params: [p1, p2],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        page_id: row.try_get(0)?,
                        wiki_id: row.try_get(1)?,
                        revision: row.try_get(2)?,
                        operation: row.try_get(3)?,
                        path: row.try_get(4)?,
                        title: row.try_get(5)?,
                        content: row.try_get(6)?,
                        agent_id: row.try_get(7)?,
                        thread_id: row.try_get(8)?,
                        at: row.try_get(9)?,
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
