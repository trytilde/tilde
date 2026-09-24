// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub connection_id: uuid::Uuid,
    pub agent_id: uuid::Uuid,
    pub identity_id: uuid::Uuid,
    pub token_hash: Vec<u8>,
    pub status: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub value: String,
    pub identity_type: String,
    pub account_name: String,
    pub provider_name: String,
    pub icon_url: Option<String>,
    pub agent_name: String,
    pub agent_identity_type: Option<String>,
    pub agent_identity_value: Option<String>,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub connection_id: uuid::Uuid,
    pub agent_id: uuid::Uuid,
    pub identity_id: uuid::Uuid,
    pub token_hash: &'a [u8],
    pub status: &'a str,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub value: &'a str,
    pub identity_type: &'a str,
    pub account_name: &'a str,
    pub provider_name: &'a str,
    pub icon_url: Option<&'a str>,
    pub agent_name: &'a str,
    pub agent_identity_type: Option<&'a str>,
    pub agent_identity_value: Option<&'a str>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            connection_id,
            agent_id,
            identity_id,
            token_hash,
            status,
            expires_at,
            value,
            identity_type,
            account_name,
            provider_name,
            icon_url,
            agent_name,
            agent_identity_type,
            agent_identity_value,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            connection_id,
            agent_id,
            identity_id,
            token_hash: token_hash.into(),
            status: status.into(),
            expires_at,
            value: value.into(),
            identity_type: identity_type.into(),
            account_name: account_name.into(),
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
        "SELECT v.id,v.connection_id,v.agent_id,v.identity_id,v.token_hash,v.status,v.expires_at,i.value,i.identity_type,c.name AS account_name,p.name AS provider_name,p.icon_url,a.name AS agent_name, ai.identity_type AS agent_identity_type, ai.value AS agent_identity_value FROM chat_identity_verifications v JOIN chat_channel_identities i ON i.id=v.identity_id JOIN connections c ON c.id=v.connection_id JOIN connection_providers p ON p.provider_id=c.provider_id JOIN agents a ON a.id=v.agent_id LEFT JOIN agent_channel_identities ai ON ai.connection_id=v.connection_id AND ai.agent_id=v.agent_id WHERE v.id=$1 AND a.deleted_at IS NULL",
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
    ) -> RecordQuery<'c, 'a, 's, C, Record, 1> {
        RecordQuery {
            client,
            params: [p1],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        id: row.try_get(0)?,
                        connection_id: row.try_get(1)?,
                        agent_id: row.try_get(2)?,
                        identity_id: row.try_get(3)?,
                        token_hash: row.try_get(4)?,
                        status: row.try_get(5)?,
                        expires_at: row.try_get(6)?,
                        value: row.try_get(7)?,
                        identity_type: row.try_get(8)?,
                        account_name: row.try_get(9)?,
                        provider_name: row.try_get(10)?,
                        icon_url: row.try_get(11)?,
                        agent_name: row.try_get(12)?,
                        agent_identity_type: row.try_get(13)?,
                        agent_identity_value: row.try_get(14)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
