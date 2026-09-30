// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub prompt_id: uuid::Uuid,
    pub number: i32,
    pub hash: Vec<u8>,
    pub template: String,
    pub config: String,
    pub variables: Vec<String>,
    pub format: String,
    pub origin: String,
    pub deployment_id: Option<uuid::Uuid>,
    pub commit_sha: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub prompt_id: uuid::Uuid,
    pub number: i32,
    pub hash: &'a [u8],
    pub template: &'a str,
    pub config: &'a str,
    pub variables: crate::ArrayIterator<'a, &'a str>,
    pub format: &'a str,
    pub origin: &'a str,
    pub deployment_id: Option<uuid::Uuid>,
    pub commit_sha: Option<&'a str>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            prompt_id,
            number,
            hash,
            template,
            config,
            variables,
            format,
            origin,
            deployment_id,
            commit_sha,
            created_at,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            prompt_id,
            number,
            hash: hash.into(),
            template: template.into(),
            config: config.into(),
            variables: variables.map(|v| v.into()).collect(),
            format: format.into(),
            origin: origin.into(),
            deployment_id,
            commit_sha: commit_sha.map(|v| v.into()),
            created_at,
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
        "SELECT v.id,v.prompt_id,v.number,v.hash,v.template,v.config,v.variables,v.format,v.origin,v.deployment_id,d.commit_sha,v.created_at FROM prompt_versions v LEFT JOIN agent_deployments d ON d.id=v.deployment_id WHERE v.prompt_id=$1 ORDER BY v.number DESC",
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
                        prompt_id: row.try_get(1)?,
                        number: row.try_get(2)?,
                        hash: row.try_get(3)?,
                        template: row.try_get(4)?,
                        config: row.try_get(5)?,
                        variables: row.try_get(6)?,
                        format: row.try_get(7)?,
                        origin: row.try_get(8)?,
                        deployment_id: row.try_get(9)?,
                        commit_sha: row.try_get(10)?,
                        created_at: row.try_get(11)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
