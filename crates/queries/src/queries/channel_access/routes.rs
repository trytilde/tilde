// This file was generated with `cornucopia`. Do not modify.

#[derive(Clone, Copy, Debug)]
pub struct RunParams {
    pub p1: uuid::Uuid,
    pub p2: Option<uuid::Uuid>,
    pub p3: i64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub connection_id: uuid::Uuid,
    pub agent_id: uuid::Uuid,
    pub access_mode: String,
    pub name: String,
    pub provider_id: String,
    pub type_id: String,
    pub status: String,
    pub provider_name: String,
    pub icon_url: Option<String>,
    pub agent_name: String,
    pub agent_identity_type: Option<String>,
    pub agent_identity_value: Option<String>,
}
pub struct RecordBorrowed<'a> {
    pub connection_id: uuid::Uuid,
    pub agent_id: uuid::Uuid,
    pub access_mode: &'a str,
    pub name: &'a str,
    pub provider_id: &'a str,
    pub type_id: &'a str,
    pub status: &'a str,
    pub provider_name: &'a str,
    pub icon_url: Option<&'a str>,
    pub agent_name: &'a str,
    pub agent_identity_type: Option<&'a str>,
    pub agent_identity_value: Option<&'a str>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            connection_id,
            agent_id,
            access_mode,
            name,
            provider_id,
            type_id,
            status,
            provider_name,
            icon_url,
            agent_name,
            agent_identity_type,
            agent_identity_value,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            connection_id,
            agent_id,
            access_mode: access_mode.into(),
            name: name.into(),
            provider_id: provider_id.into(),
            type_id: type_id.into(),
            status: status.into(),
            provider_name: provider_name.into(),
            icon_url: icon_url.map(|v| v.into()),
            agent_name: agent_name.into(),
            agent_identity_type: agent_identity_type.map(|v| v.into()),
            agent_identity_value: agent_identity_value.map(|v| v.into()),
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
        "SELECT ca.connection_id,ca.agent_id,ca.access_mode,c.name,c.provider_id,c.type_id,c.status,p.name AS provider_name,p.icon_url,a.name AS agent_name, ai.identity_type AS agent_identity_type, ai.value AS agent_identity_value FROM connection_agents ca JOIN connections c ON c.id=ca.connection_id JOIN connection_providers p ON p.provider_id=c.provider_id JOIN agents a ON a.id=ca.agent_id AND a.deleted_at IS NULL LEFT JOIN agent_channel_identities ai ON ai.connection_id=ca.connection_id AND ai.agent_id=ca.agent_id WHERE ca.agent_id=$1 AND ca.capability='channel' AND ($2::UUID IS NULL OR ca.connection_id>$2) ORDER BY ca.connection_id LIMIT $3",
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
        p2: &'a Option<uuid::Uuid>,
        p3: &'a i64,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 3> {
        RecordQuery {
            client,
            params: [p1, p2, p3],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        connection_id: row.try_get(0)?,
                        agent_id: row.try_get(1)?,
                        access_mode: row.try_get(2)?,
                        name: row.try_get(3)?,
                        provider_id: row.try_get(4)?,
                        type_id: row.try_get(5)?,
                        status: row.try_get(6)?,
                        provider_name: row.try_get(7)?,
                        icon_url: row.try_get(8)?,
                        agent_name: row.try_get(9)?,
                        agent_identity_type: row.try_get(10)?,
                        agent_identity_value: row.try_get(11)?,
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
        self.bind(client, &params.p1, &params.p2, &params.p3)
    }
}
