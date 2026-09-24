// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::StringSql, T2: crate::StringSql, T3: crate::ArraySql<Item = T2>> {
    pub p1: Option<chrono::DateTime<chrono::Utc>>,
    pub p2: Option<uuid::Uuid>,
    pub p4: T1,
    pub is_admin: bool,
    pub caller_user: Option<uuid::Uuid>,
    pub caller_key: Option<uuid::Uuid>,
    pub caller_groups: T3,
    pub p3: i64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub name: String,
    pub concurrency_policy: String,
    pub avatar_seed: uuid::Uuid,
    pub avatar_key: Option<String>,
    pub paused: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub capabilities: serde_json::Value,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub name: &'a str,
    pub concurrency_policy: &'a str,
    pub avatar_seed: uuid::Uuid,
    pub avatar_key: Option<&'a str>,
    pub paused: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub capabilities: postgres_types::Json<&'a serde_json::value::RawValue>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            name,
            concurrency_policy,
            avatar_seed,
            avatar_key,
            paused,
            created_at,
            updated_at,
            capabilities,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            concurrency_policy: concurrency_policy.into(),
            avatar_seed,
            avatar_key: avatar_key.map(|v| v.into()),
            paused,
            created_at,
            updated_at,
            capabilities: serde_json::from_str(capabilities.0.get()).unwrap(),
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
        "SELECT id, name, concurrency_policy AS concurrency_policy, avatar_seed, avatar_key, paused, created_at, updated_at, capabilities AS capabilities FROM agents WHERE deleted_at IS NULL AND ($1::TIMESTAMPTZ IS NULL OR (created_at, id) < ($1, $2)) AND ($3 = '' OR strpos(lower(name), lower($3)) > 0 OR id::text = $3) AND iam_held('agent', id, ARRAY['view']::TEXT[], $4, $5, $6, $7::TEXT[]) ORDER BY created_at DESC, id DESC LIMIT $8",
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
        T3: crate::ArraySql<Item = T2>,
    >(
        &'s self,
        client: &'c C,
        p1: &'a Option<chrono::DateTime<chrono::Utc>>,
        p2: &'a Option<uuid::Uuid>,
        p4: &'a T1,
        is_admin: &'a bool,
        caller_user: &'a Option<uuid::Uuid>,
        caller_key: &'a Option<uuid::Uuid>,
        caller_groups: &'a T3,
        p3: &'a i64,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 8> {
        RecordQuery {
            client,
            params: [
                p1,
                p2,
                p4,
                is_admin,
                caller_user,
                caller_key,
                caller_groups,
                p3,
            ],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        id: row.try_get(0)?,
                        name: row.try_get(1)?,
                        concurrency_policy: row.try_get(2)?,
                        avatar_seed: row.try_get(3)?,
                        avatar_key: row.try_get(4)?,
                        paused: row.try_get(5)?,
                        created_at: row.try_get(6)?,
                        updated_at: row.try_get(7)?,
                        capabilities: row.try_get(8)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
impl<
    'c,
    'a,
    's,
    C: GenericClient,
    T1: crate::StringSql,
    T2: crate::StringSql,
    T3: crate::ArraySql<Item = T2>,
>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        RunParams<T1, T2, T3>,
        RecordQuery<'c, 'a, 's, C, Record, 8>,
        C,
    > for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1, T2, T3>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 8> {
        self.bind(
            client,
            &params.p1,
            &params.p2,
            &params.p4,
            &params.is_admin,
            &params.caller_user,
            &params.caller_key,
            &params.caller_groups,
            &params.p3,
        )
    }
}
