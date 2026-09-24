// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub instance_id: uuid::Uuid,
    pub deployment_id: uuid::Uuid,
    pub public_url: String,
    pub runtime_url: String,
    pub ready: bool,
    pub agent_ready: bool,
    pub agent_connected: bool,
    pub connection_id: Option<uuid::Uuid>,
    pub last_seen_at: chrono::DateTime<chrono::Utc>,
}
pub struct RecordBorrowed<'a> {
    pub instance_id: uuid::Uuid,
    pub deployment_id: uuid::Uuid,
    pub public_url: &'a str,
    pub runtime_url: &'a str,
    pub ready: bool,
    pub agent_ready: bool,
    pub agent_connected: bool,
    pub connection_id: Option<uuid::Uuid>,
    pub last_seen_at: chrono::DateTime<chrono::Utc>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            instance_id,
            deployment_id,
            public_url,
            runtime_url,
            ready,
            agent_ready,
            agent_connected,
            connection_id,
            last_seen_at,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            instance_id,
            deployment_id,
            public_url: public_url.into(),
            runtime_url: runtime_url.into(),
            ready,
            agent_ready,
            agent_connected,
            connection_id,
            last_seen_at,
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
        "SELECT instance_id,deployment_id,public_url,runtime_url,ready,agent_ready,agent_connected,connection_id,last_seen_at FROM agent_instances WHERE agent_id=$1 ORDER BY instance_id",
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
                        instance_id: row.try_get(0)?,
                        deployment_id: row.try_get(1)?,
                        public_url: row.try_get(2)?,
                        runtime_url: row.try_get(3)?,
                        ready: row.try_get(4)?,
                        agent_ready: row.try_get(5)?,
                        agent_connected: row.try_get(6)?,
                        connection_id: row.try_get(7)?,
                        last_seen_at: row.try_get(8)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
