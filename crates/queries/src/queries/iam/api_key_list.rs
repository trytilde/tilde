// This file was generated with `cornucopia`. Do not modify.

#[derive(Clone, Copy, Debug)]
pub struct RunParams {
    pub after: Option<uuid::Uuid>,
    pub every: bool,
    pub owner_user: Option<uuid::Uuid>,
    pub limit_count: i64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub name: String,
    pub prefix: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub revoked_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_by_user_id: Option<uuid::Uuid>,
    pub role_ids: Vec<String>,
    pub role_names: Vec<String>,
    pub role_descriptions: Vec<String>,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub name: &'a str,
    pub prefix: &'a str,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub revoked_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_by_user_id: Option<uuid::Uuid>,
    pub role_ids: crate::ArrayIterator<'a, &'a str>,
    pub role_names: crate::ArrayIterator<'a, &'a str>,
    pub role_descriptions: crate::ArrayIterator<'a, &'a str>,
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
            role_ids,
            role_names,
            role_descriptions,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            prefix: prefix.into(),
            created_at,
            revoked_at,
            created_by_user_id,
            role_ids: role_ids.map(|v| v.into()).collect(),
            role_names: role_names.map(|v| v.into()).collect(),
            role_descriptions: role_descriptions.map(|v| v.into()).collect(),
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
        "SELECT k.id,k.name,k.prefix,k.created_at,k.revoked_at,k.created_by_user_id, COALESCE((SELECT array_agg(r.id ORDER BY r.created_at,r.id) FROM iam_role_members m JOIN iam_roles r ON r.id=m.role_id WHERE m.api_key_id=k.id),'{}'::TEXT[]) AS role_ids, COALESCE((SELECT array_agg(r.name ORDER BY r.created_at,r.id) FROM iam_role_members m JOIN iam_roles r ON r.id=m.role_id WHERE m.api_key_id=k.id),'{}'::TEXT[]) AS role_names, COALESCE((SELECT array_agg(r.description ORDER BY r.created_at,r.id) FROM iam_role_members m JOIN iam_roles r ON r.id=m.role_id WHERE m.api_key_id=k.id),'{}'::TEXT[]) AS role_descriptions FROM iam_api_keys k WHERE ($1::UUID IS NULL OR k.id>$1) AND ($2 OR k.created_by_user_id=$3) ORDER BY k.id LIMIT $4",
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
        after: &'a Option<uuid::Uuid>,
        every: &'a bool,
        owner_user: &'a Option<uuid::Uuid>,
        limit_count: &'a i64,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 4> {
        RecordQuery {
            client,
            params: [after, every, owner_user, limit_count],
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
                        role_ids: row.try_get(6)?,
                        role_names: row.try_get(7)?,
                        role_descriptions: row.try_get(8)?,
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
        self.bind(
            client,
            &params.after,
            &params.every,
            &params.owner_user,
            &params.limit_count,
        )
    }
}
