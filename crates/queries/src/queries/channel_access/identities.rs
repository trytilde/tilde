// This file was generated with `cornucopia`. Do not modify.

#[derive(Clone, Copy, Debug)]
pub struct RunParams {
    pub p1: uuid::Uuid,
    pub p2: Option<uuid::Uuid>,
    pub p3: Option<uuid::Uuid>,
    pub p4: i64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub connection_id: uuid::Uuid,
    pub value: String,
    pub identity_type: String,
    pub verified_at: Option<chrono::DateTime<chrono::Utc>>,
    pub attested_at: Option<chrono::DateTime<chrono::Utc>>,
    pub allowed: bool,
    pub verification_id: Option<uuid::Uuid>,
    pub verification_status: Option<String>,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub connection_id: uuid::Uuid,
    pub value: &'a str,
    pub identity_type: &'a str,
    pub verified_at: Option<chrono::DateTime<chrono::Utc>>,
    pub attested_at: Option<chrono::DateTime<chrono::Utc>>,
    pub allowed: bool,
    pub verification_id: Option<uuid::Uuid>,
    pub verification_status: Option<&'a str>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            connection_id,
            value,
            identity_type,
            verified_at,
            attested_at,
            allowed,
            verification_id,
            verification_status,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            connection_id,
            value: value.into(),
            identity_type: identity_type.into(),
            verified_at,
            attested_at,
            allowed,
            verification_id,
            verification_status: verification_status.map(|v| v.into()),
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
        "SELECT i.id,i.connection_id,i.value,i.identity_type,i.verified_at,u.attested_at,COALESCE(a.allowed,FALSE) AS allowed, v.id AS verification_id, CASE WHEN v.status<>'approved' AND v.expires_at<=NOW() THEN 'expired' ELSE v.status END AS verification_status FROM chat_channel_identities i JOIN chat_users u ON u.id=i.id JOIN connection_agents ca ON ca.connection_id=i.connection_id AND ca.agent_id=$1 AND ca.capability='channel' LEFT JOIN chat_channel_identity_access a ON a.identity_id=i.id AND a.agent_id=ca.agent_id AND a.connection_id=i.connection_id LEFT JOIN LATERAL (SELECT id,status,expires_at FROM chat_identity_verifications WHERE identity_id=i.id AND agent_id=$1 ORDER BY created_at DESC LIMIT 1) v ON TRUE WHERE ($2::UUID IS NULL OR i.connection_id=$2) AND ($3::UUID IS NULL OR i.id>$3) ORDER BY i.id LIMIT $4",
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
        p3: &'a Option<uuid::Uuid>,
        p4: &'a i64,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 4> {
        RecordQuery {
            client,
            params: [p1, p2, p3, p4],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        id: row.try_get(0)?,
                        connection_id: row.try_get(1)?,
                        value: row.try_get(2)?,
                        identity_type: row.try_get(3)?,
                        verified_at: row.try_get(4)?,
                        attested_at: row.try_get(5)?,
                        allowed: row.try_get(6)?,
                        verification_id: row.try_get(7)?,
                        verification_status: row.try_get(8)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient>
    crate::client::async_::Params<'c, 'a, 's, RunParams, RecordQuery<'c, 'a, 's, C, Record, 4>, C>
    for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 4> {
        self.bind(client, &params.p1, &params.p2, &params.p3, &params.p4)
    }
}
