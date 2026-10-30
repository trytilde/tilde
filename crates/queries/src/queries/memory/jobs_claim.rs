// This file was generated with `cornucopia`. Do not modify.

#[derive(Clone, Copy, Debug)]
pub struct RunParams {
    pub p2: f64,
    pub p1: i64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub memory_id: uuid::Uuid,
    pub requested_at: chrono::DateTime<chrono::Utc>,
    pub attempts: i32,
    pub title: String,
    pub content: String,
}
pub struct RunBorrowed<'a> {
    pub memory_id: uuid::Uuid,
    pub requested_at: chrono::DateTime<chrono::Utc>,
    pub attempts: i32,
    pub title: &'a str,
    pub content: &'a str,
}
impl<'a> From<RunBorrowed<'a>> for Run {
    fn from(
        RunBorrowed {
            memory_id,
            requested_at,
            attempts,
            title,
            content,
        }: RunBorrowed<'a>,
    ) -> Self {
        Self {
            memory_id,
            requested_at,
            attempts,
            title: title.into(),
            content: content.into(),
        }
    }
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RunQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<RunBorrowed, tokio_postgres::Error>,
    mapper: fn(RunBorrowed) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> RunQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(self, mapper: fn(RunBorrowed) -> R) -> RunQuery<'c, 'a, 's, C, R, N> {
        RunQuery {
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
        "UPDATE memory_embedding_jobs j SET run_after=NOW()+make_interval(secs=>$1::DOUBLE PRECISION),attempts=j.attempts+1 FROM memories m WHERE m.id=j.memory_id AND j.memory_id IN ( SELECT memory_id FROM memory_embedding_jobs WHERE run_after<=NOW() ORDER BY run_after LIMIT $2 FOR UPDATE SKIP LOCKED) RETURNING j.memory_id,j.requested_at,j.attempts,m.title,m.content",
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
        p1: &'a i64,
    ) -> RunQuery<'c, 'a, 's, C, Run, 2> {
        RunQuery {
            client,
            params: [p2, p1],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row: &tokio_postgres::Row| -> Result<RunBorrowed, tokio_postgres::Error> {
                Ok(RunBorrowed {
                    memory_id: row.try_get(0)?,
                    requested_at: row.try_get(1)?,
                    attempts: row.try_get(2)?,
                    title: row.try_get(3)?,
                    content: row.try_get(4)?,
                })
            },
            mapper: |it| Run::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient>
    crate::client::async_::Params<'c, 'a, 's, RunParams, RunQuery<'c, 'a, 's, C, Run, 2>, C>
    for RunStmt
{
    fn params(&'s self, client: &'c C, params: &'a RunParams) -> RunQuery<'c, 'a, 's, C, Run, 2> {
        self.bind(client, &params.p2, &params.p1)
    }
}
