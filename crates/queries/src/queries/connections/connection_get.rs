// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub name: String,
    pub provider_id: String,
    pub type_id: String,
    pub status: String,
    pub account_label: Option<String>,
    pub token_expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub credential_version: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub channel_capable: bool,
    pub inference_capable: bool,
    pub tool_capable: bool,
    pub signal_capable: bool,
    pub associated_agents: serde_json::Value,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub name: &'a str,
    pub provider_id: &'a str,
    pub type_id: &'a str,
    pub status: &'a str,
    pub account_label: Option<&'a str>,
    pub token_expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub credential_version: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub channel_capable: bool,
    pub inference_capable: bool,
    pub tool_capable: bool,
    pub signal_capable: bool,
    pub associated_agents: postgres_types::Json<&'a serde_json::value::RawValue>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            name,
            provider_id,
            type_id,
            status,
            account_label,
            token_expires_at,
            credential_version,
            created_at,
            updated_at,
            channel_capable,
            inference_capable,
            tool_capable,
            signal_capable,
            associated_agents,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            provider_id: provider_id.into(),
            type_id: type_id.into(),
            status: status.into(),
            account_label: account_label.map(|v| v.into()),
            token_expires_at,
            credential_version,
            created_at,
            updated_at,
            channel_capable,
            inference_capable,
            tool_capable,
            signal_capable,
            associated_agents: serde_json::from_str(associated_agents.0.get()).unwrap(),
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
        "SELECT c.id,c.name,c.provider_id,c.type_id,c.status,c.account_label,c.token_expires_at,c.credential_version,c.created_at,c.updated_at,t.channel_capable,t.inference_capable,t.tool_capable,t.signal_capable, COALESCE((SELECT jsonb_agg(jsonb_build_object('capability',ca.capability,'id',a.id,'name',a.name,'alias',ca.alias) ORDER BY ca.capability,a.id) FROM connection_agents ca JOIN agents a ON a.id=ca.agent_id WHERE ca.connection_id=c.id),'[]'::jsonb) AS associated_agents FROM connections c JOIN connection_types t ON(t.provider_id=c.provider_id AND t.type_id=c.type_id) WHERE c.id=$1",
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
                        name: row.try_get(1)?,
                        provider_id: row.try_get(2)?,
                        type_id: row.try_get(3)?,
                        status: row.try_get(4)?,
                        account_label: row.try_get(5)?,
                        token_expires_at: row.try_get(6)?,
                        credential_version: row.try_get(7)?,
                        created_at: row.try_get(8)?,
                        updated_at: row.try_get(9)?,
                        channel_capable: row.try_get(10)?,
                        inference_capable: row.try_get(11)?,
                        tool_capable: row.try_get(12)?,
                        signal_capable: row.try_get(13)?,
                        associated_agents: row.try_get(14)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
