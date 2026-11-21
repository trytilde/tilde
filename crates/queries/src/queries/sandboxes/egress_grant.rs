// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::StringSql, T2: crate::BytesSql> {
    pub p2: T1,
    pub p1: T2,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub sandbox_id: uuid::Uuid,
    pub connection_id: uuid::Uuid,
    pub provider_id: String,
    pub type_id: String,
    pub config: serde_json::Value,
    pub config_version: i32,
}
pub struct RecordBorrowed<'a> {
    pub sandbox_id: uuid::Uuid,
    pub connection_id: uuid::Uuid,
    pub provider_id: &'a str,
    pub type_id: &'a str,
    pub config: postgres_types::Json<&'a serde_json::value::RawValue>,
    pub config_version: i32,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            sandbox_id,
            connection_id,
            provider_id,
            type_id,
            config,
            config_version,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            sandbox_id,
            connection_id,
            provider_id: provider_id.into(),
            type_id: type_id.into(),
            config: serde_json::from_str(config.0.get()).unwrap(),
            config_version,
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
        "SELECT s.id AS sandbox_id,g.connection_id,c.provider_id,c.type_id,g.config,g.config_version FROM sandboxes s JOIN sandbox_blueprint_egress g ON g.blueprint_id=s.blueprint_id AND g.slug=$1 AND g.enabled JOIN connections c ON c.id=g.connection_id AND c.status='ready' AND c.owner_user_id IS NULL WHERE s.egress_hash=$2",
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
    pub fn bind<'c, 'a, 's, C: GenericClient, T1: crate::StringSql, T2: crate::BytesSql>(
        &'s self,
        client: &'c C,
        p2: &'a T1,
        p1: &'a T2,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        RecordQuery {
            client,
            params: [p2, p1],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        sandbox_id: row.try_get(0)?,
                        connection_id: row.try_get(1)?,
                        provider_id: row.try_get(2)?,
                        type_id: row.try_get(3)?,
                        config: row.try_get(4)?,
                        config_version: row.try_get(5)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::StringSql, T2: crate::BytesSql>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        RunParams<T1, T2>,
        RecordQuery<'c, 'a, 's, C, Record, 2>,
        C,
    > for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1, T2>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        self.bind(client, &params.p2, &params.p1)
    }
}
