// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::StringSql> {
    pub kind: T1,
    pub resource_id: uuid::Uuid,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub id: String,
    pub name: String,
    pub description: String,
}
pub struct RunBorrowed<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub description: &'a str,
}
impl<'a> From<RunBorrowed<'a>> for Run {
    fn from(
        RunBorrowed {
            id,
            name,
            description,
        }: RunBorrowed<'a>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
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
        "SELECT r.id,r.name,r.description FROM iam_roles r WHERE EXISTS(SELECT 1 FROM iam_role_statements s WHERE s.role_id=r.id AND s.resource_kind=$1 AND s.resource_id=$2) ORDER BY r.created_at,r.id",
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
    pub fn bind<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>(
        &'s self,
        client: &'c C,
        kind: &'a T1,
        resource_id: &'a uuid::Uuid,
    ) -> RunQuery<'c, 'a, 's, C, Run, 2> {
        RunQuery {
            client,
            params: [kind, resource_id],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row: &tokio_postgres::Row| -> Result<RunBorrowed, tokio_postgres::Error> {
                Ok(RunBorrowed {
                    id: row.try_get(0)?,
                    name: row.try_get(1)?,
                    description: row.try_get(2)?,
                })
            },
            mapper: |it| Run::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>
    crate::client::async_::Params<'c, 'a, 's, RunParams<T1>, RunQuery<'c, 'a, 's, C, Run, 2>, C>
    for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1>,
    ) -> RunQuery<'c, 'a, 's, C, Run, 2> {
        self.bind(client, &params.kind, &params.resource_id)
    }
}
