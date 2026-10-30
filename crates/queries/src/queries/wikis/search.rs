// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::StringSql, T2: crate::ArraySql<Item = uuid::Uuid>> {
    pub p2: T1,
    pub p1: T2,
    pub p3: i64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub id: uuid::Uuid,
    pub wiki_id: uuid::Uuid,
    pub wiki_name: String,
    pub path: String,
    pub title: String,
    pub revision: i32,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub rank: f32,
    pub excerpt: String,
}
pub struct RunBorrowed<'a> {
    pub id: uuid::Uuid,
    pub wiki_id: uuid::Uuid,
    pub wiki_name: &'a str,
    pub path: &'a str,
    pub title: &'a str,
    pub revision: i32,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub rank: f32,
    pub excerpt: &'a str,
}
impl<'a> From<RunBorrowed<'a>> for Run {
    fn from(
        RunBorrowed {
            id,
            wiki_id,
            wiki_name,
            path,
            title,
            revision,
            updated_at,
            rank,
            excerpt,
        }: RunBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            wiki_id,
            wiki_name: wiki_name.into(),
            path: path.into(),
            title: title.into(),
            revision,
            updated_at,
            rank,
            excerpt: excerpt.into(),
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
        "WITH q AS ( SELECT replace(plainto_tsquery('english', $1)::TEXT, ' & ', ' | ')::tsquery AS tsq ) SELECT p.id,p.wiki_id,w.name AS wiki_name,p.path,p.title,p.revision,p.updated_at, ts_rank_cd(p.search, q.tsq) AS rank, ts_headline('english', p.content, q.tsq, 'MaxFragments=2, MaxWords=24, MinWords=8, StartSel=\u{e000}, StopSel=\u{e001}') AS excerpt FROM wiki_pages p JOIN wikis w ON w.id=p.wiki_id, q WHERE p.wiki_id = ANY($2) AND numnode(q.tsq) > 0 AND p.search @@ q.tsq ORDER BY rank DESC, p.path LIMIT $3",
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
    pub fn bind<
        'c,
        'a,
        's,
        C: GenericClient,
        T1: crate::StringSql,
        T2: crate::ArraySql<Item = uuid::Uuid>,
    >(
        &'s self,
        client: &'c C,
        p2: &'a T1,
        p1: &'a T2,
        p3: &'a i64,
    ) -> RunQuery<'c, 'a, 's, C, Run, 3> {
        RunQuery {
            client,
            params: [p2, p1, p3],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row: &tokio_postgres::Row| -> Result<RunBorrowed, tokio_postgres::Error> {
                Ok(RunBorrowed {
                    id: row.try_get(0)?,
                    wiki_id: row.try_get(1)?,
                    wiki_name: row.try_get(2)?,
                    path: row.try_get(3)?,
                    title: row.try_get(4)?,
                    revision: row.try_get(5)?,
                    updated_at: row.try_get(6)?,
                    rank: row.try_get(7)?,
                    excerpt: row.try_get(8)?,
                })
            },
            mapper: |it| Run::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::StringSql, T2: crate::ArraySql<Item = uuid::Uuid>>
    crate::client::async_::Params<'c, 'a, 's, RunParams<T1, T2>, RunQuery<'c, 'a, 's, C, Run, 3>, C>
    for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1, T2>,
    ) -> RunQuery<'c, 'a, 's, C, Run, 3> {
        self.bind(client, &params.p2, &params.p1, &params.p3)
    }
}
