// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::StringSql, T2: crate::StringSql, T3: crate::BytesSql> {
    pub id: uuid::Uuid,
    pub name: T1,
    pub prefix: T2,
    pub token_hash: T3,
    pub user_id: uuid::Uuid,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub name: String,
    pub prefix: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub revoked_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_by_user_id: Option<uuid::Uuid>,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub name: &'a str,
    pub prefix: &'a str,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub revoked_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_by_user_id: Option<uuid::Uuid>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            name,
            prefix,
            created_at,
            revoked_at,
            created_by_user_id,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            prefix: prefix.into(),
            created_at,
            revoked_at,
            created_by_user_id,
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
        "INSERT INTO iam_api_keys(id,name,prefix,token_hash,created_by_user_id) VALUES($1,$2,$3,$4,$5) RETURNING id,name,prefix,created_at,revoked_at,created_by_user_id",
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
    pub fn bind<
        'c,
        'a,
        's,
        C: GenericClient,
        T1: crate::StringSql,
        T2: crate::StringSql,
        T3: crate::BytesSql,
    >(
        &'s self,
        client: &'c C,
        id: &'a uuid::Uuid,
        name: &'a T1,
        prefix: &'a T2,
        token_hash: &'a T3,
        user_id: &'a uuid::Uuid,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 5> {
        RecordQuery {
            client,
            params: [id, name, prefix, token_hash, user_id],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        id: row.try_get(0)?,
                        name: row.try_get(1)?,
                        prefix: row.try_get(2)?,
                        created_at: row.try_get(3)?,
                        revoked_at: row.try_get(4)?,
                        created_by_user_id: row.try_get(5)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::StringSql, T2: crate::StringSql, T3: crate::BytesSql>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        RunParams<T1, T2, T3>,
        RecordQuery<'c, 'a, 's, C, Record, 5>,
        C,
    > for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1, T2, T3>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 5> {
        self.bind(
            client,
            &params.id,
            &params.name,
            &params.prefix,
            &params.token_hash,
            &params.user_id,
        )
    }
}
