// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub provider_id: String,
    pub type_id: String,
    pub name: String,
    pub driver: String,
    pub channel_capable: bool,
    pub authorization_url: Option<String>,
    pub token_url: Option<String>,
    pub client_auth: String,
    pub pkce: bool,
    pub scopes: Vec<String>,
    pub scope_separator: String,
    pub access_token_path: String,
    pub refresh_token_path: String,
    pub expires_in_path: String,
    pub scope_path: String,
    pub success_path: Option<String>,
    pub credential_schema: Option<serde_json::Value>,
    pub inference_capable: bool,
}
pub struct RecordBorrowed<'a> {
    pub provider_id: &'a str,
    pub type_id: &'a str,
    pub name: &'a str,
    pub driver: &'a str,
    pub channel_capable: bool,
    pub authorization_url: Option<&'a str>,
    pub token_url: Option<&'a str>,
    pub client_auth: &'a str,
    pub pkce: bool,
    pub scopes: crate::ArrayIterator<'a, &'a str>,
    pub scope_separator: &'a str,
    pub access_token_path: &'a str,
    pub refresh_token_path: &'a str,
    pub expires_in_path: &'a str,
    pub scope_path: &'a str,
    pub success_path: Option<&'a str>,
    pub credential_schema: Option<postgres_types::Json<&'a serde_json::value::RawValue>>,
    pub inference_capable: bool,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            provider_id,
            type_id,
            name,
            driver,
            channel_capable,
            authorization_url,
            token_url,
            client_auth,
            pkce,
            scopes,
            scope_separator,
            access_token_path,
            refresh_token_path,
            expires_in_path,
            scope_path,
            success_path,
            credential_schema,
            inference_capable,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            type_id: type_id.into(),
            name: name.into(),
            driver: driver.into(),
            channel_capable,
            authorization_url: authorization_url.map(|v| v.into()),
            token_url: token_url.map(|v| v.into()),
            client_auth: client_auth.into(),
            pkce,
            scopes: scopes.map(|v| v.into()).collect(),
            scope_separator: scope_separator.into(),
            access_token_path: access_token_path.into(),
            refresh_token_path: refresh_token_path.into(),
            expires_in_path: expires_in_path.into(),
            scope_path: scope_path.into(),
            success_path: success_path.map(|v| v.into()),
            credential_schema: credential_schema.map(|v| serde_json::from_str(v.0.get()).unwrap()),
            inference_capable,
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
        "SELECT provider_id,type_id,name,driver,channel_capable,authorization_url,token_url,client_auth,pkce,scopes,scope_separator,access_token_path,refresh_token_path,expires_in_path,scope_path,success_path,credential_schema,inference_capable FROM connection_types WHERE provider_id=$1 ORDER BY type_id",
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
        p1: &'a T1,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 1> {
        RecordQuery {
            client,
            params: [p1],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        provider_id: row.try_get(0)?,
                        type_id: row.try_get(1)?,
                        name: row.try_get(2)?,
                        driver: row.try_get(3)?,
                        channel_capable: row.try_get(4)?,
                        authorization_url: row.try_get(5)?,
                        token_url: row.try_get(6)?,
                        client_auth: row.try_get(7)?,
                        pkce: row.try_get(8)?,
                        scopes: row.try_get(9)?,
                        scope_separator: row.try_get(10)?,
                        access_token_path: row.try_get(11)?,
                        refresh_token_path: row.try_get(12)?,
                        expires_in_path: row.try_get(13)?,
                        scope_path: row.try_get(14)?,
                        success_path: row.try_get(15)?,
                        credential_schema: row.try_get(16)?,
                        inference_capable: row.try_get(17)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
