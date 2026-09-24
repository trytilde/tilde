// This file was generated with `cornucopia`. Do not modify.

#[derive(Clone, Copy, Debug)]
pub struct RunParams {
    pub p1: uuid::Uuid,
    pub p2: uuid::Uuid,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub agent_id: uuid::Uuid,
    pub source: String,
    pub target: String,
    pub target_reference: Option<String>,
    pub repository: Option<String>,
    pub commit_sha: Option<String>,
    pub external_id: Option<String>,
    pub label: String,
    pub status: String,
    pub token_issued: bool,
    pub serving: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub retired_at: Option<chrono::DateTime<chrono::Utc>>,
    pub commit_message: Option<String>,
    pub branch: Option<String>,
    pub commit_author: Option<String>,
    pub traffic_weight: i32,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub agent_id: uuid::Uuid,
    pub source: &'a str,
    pub target: &'a str,
    pub target_reference: Option<&'a str>,
    pub repository: Option<&'a str>,
    pub commit_sha: Option<&'a str>,
    pub external_id: Option<&'a str>,
    pub label: &'a str,
    pub status: &'a str,
    pub token_issued: bool,
    pub serving: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub retired_at: Option<chrono::DateTime<chrono::Utc>>,
    pub commit_message: Option<&'a str>,
    pub branch: Option<&'a str>,
    pub commit_author: Option<&'a str>,
    pub traffic_weight: i32,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            agent_id,
            source,
            target,
            target_reference,
            repository,
            commit_sha,
            external_id,
            label,
            status,
            token_issued,
            serving,
            created_at,
            retired_at,
            commit_message,
            branch,
            commit_author,
            traffic_weight,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            agent_id,
            source: source.into(),
            target: target.into(),
            target_reference: target_reference.map(|v| v.into()),
            repository: repository.map(|v| v.into()),
            commit_sha: commit_sha.map(|v| v.into()),
            external_id: external_id.map(|v| v.into()),
            label: label.into(),
            status: status.into(),
            token_issued,
            serving,
            created_at,
            retired_at,
            commit_message: commit_message.map(|v| v.into()),
            branch: branch.map(|v| v.into()),
            commit_author: commit_author.map(|v| v.into()),
            traffic_weight,
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
        "SELECT d.id,d.agent_id,d.source,d.target,d.target_reference,d.repository,d.commit_sha,d.external_id,d.label,d.status,d.token_hash IS NOT NULL AS token_issued, CASE WHEN s.routing='weighted' THEN d.status='registered' AND d.traffic_weight>0 ELSE COALESCE(d.id=s.serving_deployment_id,false) END AS serving,d.created_at,d.retired_at,d.commit_message,d.branch,d.commit_author,d.traffic_weight FROM agent_deployments d JOIN agent_deployment_settings s ON s.agent_id=d.agent_id WHERE d.id=$1 AND d.agent_id=$2",
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
        p2: &'a uuid::Uuid,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        RecordQuery {
            client,
            params: [p1, p2],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        id: row.try_get(0)?,
                        agent_id: row.try_get(1)?,
                        source: row.try_get(2)?,
                        target: row.try_get(3)?,
                        target_reference: row.try_get(4)?,
                        repository: row.try_get(5)?,
                        commit_sha: row.try_get(6)?,
                        external_id: row.try_get(7)?,
                        label: row.try_get(8)?,
                        status: row.try_get(9)?,
                        token_issued: row.try_get(10)?,
                        serving: row.try_get(11)?,
                        created_at: row.try_get(12)?,
                        retired_at: row.try_get(13)?,
                        commit_message: row.try_get(14)?,
                        branch: row.try_get(15)?,
                        commit_author: row.try_get(16)?,
                        traffic_weight: row.try_get(17)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient>
    crate::client::async_::Params<'c, 'a, 's, RunParams, RecordQuery<'c, 'a, 's, C, Record, 2>, C>
    for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        self.bind(client, &params.p1, &params.p2)
    }
}
