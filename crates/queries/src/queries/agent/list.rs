// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::StringSql, T2: crate::StringSql> {
    pub fresh_after: chrono::DateTime<chrono::Utc>,
    pub health: T1,
    pub p1: Option<chrono::DateTime<chrono::Utc>>,
    pub p2: Option<uuid::Uuid>,
    pub p4: T2,
    pub paused: Option<bool>,
    pub p3: i64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub name: String,
    pub description: String,
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
    pub description: &'a str,
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
            description,
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
            description: description.into(),
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
        "SELECT a.id, a.name, a.description, a.concurrency_policy AS concurrency_policy, a.avatar_seed, a.avatar_key, a.paused, a.created_at, a.updated_at, a.capabilities AS capabilities FROM agents a LEFT JOIN LATERAL ( SELECT h.healthy, h.degraded FROM agent_health h WHERE h.agent_id = a.id AND h.sidecar_event_id IS NULL AND h.checked_at >= $1 ORDER BY h.checked_at DESC, h.id DESC LIMIT 1 ) latest ON $2 <> '' WHERE a.deleted_at IS NULL AND ($3::TIMESTAMPTZ IS NULL OR (a.created_at, a.id) < ($3, $4)) AND ($5 = '' OR strpos(lower(a.name), lower($5)) > 0 OR strpos(lower(a.description), lower($5)) > 0 OR a.id::text = $5) AND ($6::BOOLEAN IS NULL OR a.paused = $6) AND ($2 = '' OR $2 = CASE WHEN latest.healthy IS NULL THEN 'unknown' WHEN latest.degraded THEN 'degraded' WHEN latest.healthy THEN 'healthy' ELSE 'unhealthy' END) ORDER BY a.created_at DESC, a.id DESC LIMIT $7",
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
    pub fn bind<'c, 'a, 's, C: GenericClient, T1: crate::StringSql, T2: crate::StringSql>(
        &'s self,
        client: &'c C,
        fresh_after: &'a chrono::DateTime<chrono::Utc>,
        health: &'a T1,
        p1: &'a Option<chrono::DateTime<chrono::Utc>>,
        p2: &'a Option<uuid::Uuid>,
        p4: &'a T2,
        paused: &'a Option<bool>,
        p3: &'a i64,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 7> {
        RecordQuery {
            client,
            params: [fresh_after, health, p1, p2, p4, paused, p3],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        id: row.try_get(0)?,
                        name: row.try_get(1)?,
                        description: row.try_get(2)?,
                        concurrency_policy: row.try_get(3)?,
                        avatar_seed: row.try_get(4)?,
                        avatar_key: row.try_get(5)?,
                        paused: row.try_get(6)?,
                        created_at: row.try_get(7)?,
                        updated_at: row.try_get(8)?,
                        capabilities: row.try_get(9)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::StringSql, T2: crate::StringSql>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        RunParams<T1, T2>,
        RecordQuery<'c, 'a, 's, C, Record, 7>,
        C,
    > for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1, T2>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 7> {
        self.bind(
            client,
            &params.fresh_after,
            &params.health,
            &params.p1,
            &params.p2,
            &params.p4,
            &params.paused,
            &params.p3,
        )
    }
}
