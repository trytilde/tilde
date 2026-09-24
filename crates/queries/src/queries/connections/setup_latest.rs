// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub provider_redirect_url: Option<String>,
    pub id: uuid::Uuid,
    pub connection_id: uuid::Uuid,
    pub step: String,
    pub action_id: uuid::Uuid,
    pub connection_setup_token: Vec<u8>,
    pub connection_setup_token_hash: Vec<u8>,
    pub callback_token: Vec<u8>,
    pub callback_hash: Vec<u8>,
    pub error_code: Option<String>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub claimed_at: Option<chrono::DateTime<chrono::Utc>>,
}
pub struct RecordBorrowed<'a> {
    pub provider_redirect_url: Option<&'a str>,
    pub id: uuid::Uuid,
    pub connection_id: uuid::Uuid,
    pub step: &'a str,
    pub action_id: uuid::Uuid,
    pub connection_setup_token: &'a [u8],
    pub connection_setup_token_hash: &'a [u8],
    pub callback_token: &'a [u8],
    pub callback_hash: &'a [u8],
    pub error_code: Option<&'a str>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub claimed_at: Option<chrono::DateTime<chrono::Utc>>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            provider_redirect_url,
            id,
            connection_id,
            step,
            action_id,
            connection_setup_token,
            connection_setup_token_hash,
            callback_token,
            callback_hash,
            error_code,
            expires_at,
            claimed_at,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            provider_redirect_url: provider_redirect_url.map(|v| v.into()),
            id,
            connection_id,
            step: step.into(),
            action_id,
            connection_setup_token: connection_setup_token.into(),
            connection_setup_token_hash: connection_setup_token_hash.into(),
            callback_token: callback_token.into(),
            callback_hash: callback_hash.into(),
            error_code: error_code.map(|v| v.into()),
            expires_at,
            claimed_at,
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
        "SELECT provider_redirect_url,id,connection_id,step,action_id,connection_setup_token,connection_setup_token_hash,callback_token,callback_hash,error_code,expires_at,claimed_at FROM connection_setups WHERE connection_id=$1 ORDER BY created_at DESC LIMIT 1",
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
                        provider_redirect_url: row.try_get(0)?,
                        id: row.try_get(1)?,
                        connection_id: row.try_get(2)?,
                        step: row.try_get(3)?,
                        action_id: row.try_get(4)?,
                        connection_setup_token: row.try_get(5)?,
                        connection_setup_token_hash: row.try_get(6)?,
                        callback_token: row.try_get(7)?,
                        callback_hash: row.try_get(8)?,
                        error_code: row.try_get(9)?,
                        expires_at: row.try_get(10)?,
                        claimed_at: row.try_get(11)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
