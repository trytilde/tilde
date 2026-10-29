// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::StringSql, T2: crate::ArraySql<Item = uuid::Uuid>> {
    pub p2: T1,
    pub p1: T2,
    pub p4: bool,
    pub p3: Option<uuid::Uuid>,
    pub p5: i64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub path: String,
    pub count: i64,
}
pub struct RunBorrowed<'a> {
    pub path: &'a str,
    pub count: i64,
}
impl<'a> From<RunBorrowed<'a>> for Run {
    fn from(RunBorrowed { path, count }: RunBorrowed<'a>) -> Self {
        Self {
            path: path.into(),
            count,
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
        "SELECT CASE WHEN $1='' THEN split_part(m.path,'/',1) ELSE $1||'/'||split_part(substr(m.path,length($1)+2),'/',1) END AS path, COUNT(*) AS count FROM memories m JOIN memory_banks b ON b.id=m.bank_id WHERE m.bank_id=ANY($2::UUID[]) AND m.deleted_at IS NULL AND (($1='' AND m.path<>'') OR starts_with(m.path,$1||'/')) AND ($3 OR b.scope='shared' OR m.identity_id IS NULL OR m.identity_id=$4::UUID) AND NOT EXISTS (SELECT 1 FROM memories s WHERE s.supersedes_id=m.id) GROUP BY 1 ORDER BY 1 LIMIT $5",
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
        p4: &'a bool,
        p3: &'a Option<uuid::Uuid>,
        p5: &'a i64,
    ) -> RunQuery<'c, 'a, 's, C, Run, 5> {
        RunQuery {
            client,
            params: [p2, p1, p4, p3, p5],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row: &tokio_postgres::Row| -> Result<RunBorrowed, tokio_postgres::Error> {
                Ok(RunBorrowed {
                    path: row.try_get(0)?,
                    count: row.try_get(1)?,
                })
            },
            mapper: |it| Run::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::StringSql, T2: crate::ArraySql<Item = uuid::Uuid>>
    crate::client::async_::Params<'c, 'a, 's, RunParams<T1, T2>, RunQuery<'c, 'a, 's, C, Run, 5>, C>
    for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1, T2>,
    ) -> RunQuery<'c, 'a, 's, C, Run, 5> {
        self.bind(
            client, &params.p2, &params.p1, &params.p4, &params.p3, &params.p5,
        )
    }
}
