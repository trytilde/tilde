// This file was generated with `cornucopia`. Do not modify.

#[derive(Clone, Copy, Debug)]
pub struct RunParams {
    pub p3: f64,
    pub p1: uuid::Uuid,
    pub p2: uuid::Uuid,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub deployment_id: Option<uuid::Uuid>,
    pub instance_id: uuid::Uuid,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub public_url: Option<String>,
    pub live: bool,
}
pub struct RecordBorrowed<'a> {
    pub deployment_id: Option<uuid::Uuid>,
    pub instance_id: uuid::Uuid,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub public_url: Option<&'a str>,
    pub live: bool,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            deployment_id,
            instance_id,
            updated_at,
            public_url,
            live,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            deployment_id,
            instance_id,
            updated_at,
            public_url: public_url.map(|v| v.into()),
            live,
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
        "SELECT n.deployment_id AS deployment_id,l.instance_id,l.updated_at,n.public_url AS public_url,COALESCE(n.ready AND n.last_seen_at>NOW()-make_interval(secs=>$1::float8),FALSE) AS live FROM thread_leases l LEFT JOIN agent_instances n ON n.agent_id=l.agent_id AND n.instance_id=l.instance_id WHERE l.thread_id=$2 AND l.agent_id=$3",
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
        p3: &'a f64,
        p1: &'a uuid::Uuid,
        p2: &'a uuid::Uuid,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 3> {
        RecordQuery {
            client,
            params: [p3, p1, p2],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        deployment_id: row.try_get(0)?,
                        instance_id: row.try_get(1)?,
                        updated_at: row.try_get(2)?,
                        public_url: row.try_get(3)?,
                        live: row.try_get(4)?,
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
        self.bind(client, &params.p3, &params.p1, &params.p2)
    }
}
