// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub batch_id: uuid::Uuid,
    pub memory_id: uuid::Uuid,
    pub title: String,
    pub forgotten: bool,
}
pub struct RunBorrowed<'a> {
    pub batch_id: uuid::Uuid,
    pub memory_id: uuid::Uuid,
    pub title: &'a str,
    pub forgotten: bool,
}
impl<'a> From<RunBorrowed<'a>> for Run {
    fn from(
        RunBorrowed {
            batch_id,
            memory_id,
            title,
            forgotten,
        }: RunBorrowed<'a>,
    ) -> Self {
        Self {
            batch_id,
            memory_id,
            title: title.into(),
            forgotten,
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
        "SELECT DISTINCT e.batch_id,m.id AS memory_id,m.title,FALSE AS forgotten FROM memory_evidence e JOIN memories m ON m.id=e.memory_id WHERE e.batch_id=ANY($1::UUID[]) UNION SELECT r.batch_id,r.memory_id,r.title,TRUE FROM memory_revisions r WHERE r.batch_id=ANY($1::UUID[]) AND r.operation='forget' ORDER BY 1,4,3",
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
    ) -> RunQuery<'c, 'a, 's, C, Run, 1> {
        RunQuery {
            client,
            params: [p1],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row: &tokio_postgres::Row| -> Result<RunBorrowed, tokio_postgres::Error> {
                Ok(RunBorrowed {
                    batch_id: row.try_get(0)?,
                    memory_id: row.try_get(1)?,
                    title: row.try_get(2)?,
                    forgotten: row.try_get(3)?,
                })
            },
            mapper: |it| Run::from(it),
        }
    }
}
