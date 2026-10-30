// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::ArraySql<Item = uuid::Uuid>, T2: crate::StringSql> {
    pub p1: T1,
    pub p3: bool,
    pub p2: T2,
    pub p4: i64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub wiki_id: uuid::Uuid,
    pub wiki_name: String,
    pub path: String,
    pub line_number: i64,
    pub line: String,
    pub truncated: bool,
}
pub struct RunBorrowed<'a> {
    pub wiki_id: uuid::Uuid,
    pub wiki_name: &'a str,
    pub path: &'a str,
    pub line_number: i64,
    pub line: &'a str,
    pub truncated: bool,
}
impl<'a> From<RunBorrowed<'a>> for Run {
    fn from(
        RunBorrowed {
            wiki_id,
            wiki_name,
            path,
            line_number,
            line,
            truncated,
        }: RunBorrowed<'a>,
    ) -> Self {
        Self {
            wiki_id,
            wiki_name: wiki_name.into(),
            path: path.into(),
            line_number,
            line: line.into(),
            truncated,
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
        "SELECT p.wiki_id,w.name AS wiki_name,p.path,l.n AS line_number,left(l.line, 500) AS line, length(l.line) > 500 AS truncated FROM wiki_pages p JOIN wikis w ON w.id=p.wiki_id, LATERAL unnest(string_to_array(p.content, E'\\n')) WITH ORDINALITY AS l(line, n) WHERE p.wiki_id = ANY($1) AND CASE WHEN $2 THEN l.line ~* $3 ELSE l.line ~ $3 END ORDER BY p.path, l.n LIMIT $4",
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
        T1: crate::ArraySql<Item = uuid::Uuid>,
        T2: crate::StringSql,
    >(
        &'s self,
        client: &'c C,
        p1: &'a T1,
        p3: &'a bool,
        p2: &'a T2,
        p4: &'a i64,
    ) -> RunQuery<'c, 'a, 's, C, Run, 4> {
        RunQuery {
            client,
            params: [p1, p3, p2, p4],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row: &tokio_postgres::Row| -> Result<RunBorrowed, tokio_postgres::Error> {
                Ok(RunBorrowed {
                    wiki_id: row.try_get(0)?,
                    wiki_name: row.try_get(1)?,
                    path: row.try_get(2)?,
                    line_number: row.try_get(3)?,
                    line: row.try_get(4)?,
                    truncated: row.try_get(5)?,
                })
            },
            mapper: |it| Run::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::ArraySql<Item = uuid::Uuid>, T2: crate::StringSql>
    crate::client::async_::Params<'c, 'a, 's, RunParams<T1, T2>, RunQuery<'c, 'a, 's, C, Run, 4>, C>
    for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1, T2>,
    ) -> RunQuery<'c, 'a, 's, C, Run, 4> {
        self.bind(client, &params.p1, &params.p3, &params.p2, &params.p4)
    }
}
