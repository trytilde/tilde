// This file was generated with `cornucopia`. Do not modify.

#[derive(Clone, Copy, Debug)]
pub struct RunParams {
    pub p1: uuid::Uuid,
    pub p2: uuid::Uuid,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub bound_connection_id: Option<uuid::Uuid>,
    pub external_id: Option<String>,
    pub id: Option<uuid::Uuid>,
    pub name: Option<String>,
    pub provider_id: Option<String>,
    pub type_id: Option<String>,
    pub account_label: Option<String>,
}
pub struct RecordBorrowed<'a> {
    pub bound_connection_id: Option<uuid::Uuid>,
    pub external_id: Option<&'a str>,
    pub id: Option<uuid::Uuid>,
    pub name: Option<&'a str>,
    pub provider_id: Option<&'a str>,
    pub type_id: Option<&'a str>,
    pub account_label: Option<&'a str>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            bound_connection_id,
            external_id,
            id,
            name,
            provider_id,
            type_id,
            account_label,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            bound_connection_id,
            external_id: external_id.map(|v| v.into()),
            id,
            name: name.map(|v| v.into()),
            provider_id: provider_id.map(|v| v.into()),
            type_id: type_id.map(|v| v.into()),
            account_label: account_label.map(|v| v.into()),
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
        "SELECT b.connection_id AS bound_connection_id, b.external_id AS external_id, c.id AS id, c.name AS name, c.provider_id AS provider_id, c.type_id AS type_id, c.account_label AS account_label FROM chat_threads t LEFT JOIN chat_channel_threads b ON b.thread_id = t.id LEFT JOIN LATERAL ( SELECT c.id, c.name, c.provider_id, c.type_id, c.account_label FROM connection_agents ca JOIN connections c ON c.id = ca.connection_id WHERE ca.agent_id = $1 AND ca.capability = 'channel' AND c.status = 'ready' ) c ON TRUE WHERE t.id = $2 ORDER BY c.id",
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
        p2: &'a uuid::Uuid,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        RecordQuery {
            client,
            params: [p1, p2],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        bound_connection_id: row.try_get(0)?,
                        external_id: row.try_get(1)?,
                        id: row.try_get(2)?,
                        name: row.try_get(3)?,
                        provider_id: row.try_get(4)?,
                        type_id: row.try_get(5)?,
                        account_label: row.try_get(6)?,
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
