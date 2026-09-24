// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub name: String,
    pub provider_id: String,
    pub connection_id: Option<uuid::Uuid>,
    pub identity_type: String,
    pub value: String,
    pub root_identity_id: Option<uuid::Uuid>,
    pub verified_at: Option<chrono::DateTime<chrono::Utc>>,
    pub attested_at: Option<chrono::DateTime<chrono::Utc>>,
    pub attested_by_user_id: Option<uuid::Uuid>,
    pub attested_by_api_key_id: Option<uuid::Uuid>,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub name: &'a str,
    pub provider_id: &'a str,
    pub connection_id: Option<uuid::Uuid>,
    pub identity_type: &'a str,
    pub value: &'a str,
    pub root_identity_id: Option<uuid::Uuid>,
    pub verified_at: Option<chrono::DateTime<chrono::Utc>>,
    pub attested_at: Option<chrono::DateTime<chrono::Utc>>,
    pub attested_by_user_id: Option<uuid::Uuid>,
    pub attested_by_api_key_id: Option<uuid::Uuid>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            name,
            provider_id,
            connection_id,
            identity_type,
            value,
            root_identity_id,
            verified_at,
            attested_at,
            attested_by_user_id,
            attested_by_api_key_id,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            provider_id: provider_id.into(),
            connection_id,
            identity_type: identity_type.into(),
            value: value.into(),
            root_identity_id,
            verified_at,
            attested_at,
            attested_by_user_id,
            attested_by_api_key_id,
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
        "SELECT u.id,u.name,COALESCE(c.provider_id,'tilde') AS provider_id,i.connection_id, COALESCE(i.identity_type,'username') AS identity_type,COALESCE(i.value,n.value,u.id::TEXT) AS value, u.root_identity_id,i.verified_at,u.attested_at,u.attested_by_user_id,u.attested_by_api_key_id FROM chat_users u LEFT JOIN chat_channel_identities i ON i.id=u.id LEFT JOIN connections c ON c.id=i.connection_id LEFT JOIN chat_native_identities n ON n.id=u.id WHERE u.id=$1",
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
        id: &'a uuid::Uuid,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 1> {
        RecordQuery {
            client,
            params: [id],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        id: row.try_get(0)?,
                        name: row.try_get(1)?,
                        provider_id: row.try_get(2)?,
                        connection_id: row.try_get(3)?,
                        identity_type: row.try_get(4)?,
                        value: row.try_get(5)?,
                        root_identity_id: row.try_get(6)?,
                        verified_at: row.try_get(7)?,
                        attested_at: row.try_get(8)?,
                        attested_by_user_id: row.try_get(9)?,
                        attested_by_api_key_id: row.try_get(10)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
