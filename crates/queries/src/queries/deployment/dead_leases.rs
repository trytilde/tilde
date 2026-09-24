// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub thread_id: uuid::Uuid,
    pub agent_id: uuid::Uuid,
    pub instance_id: uuid::Uuid,
    pub failure_mode: String,
    pub deployment_id: Option<uuid::Uuid>,
    pub active: bool,
}
pub struct RecordBorrowed<'a> {
    pub thread_id: uuid::Uuid,
    pub agent_id: uuid::Uuid,
    pub instance_id: uuid::Uuid,
    pub failure_mode: &'a str,
    pub deployment_id: Option<uuid::Uuid>,
    pub active: bool,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            thread_id,
            agent_id,
            instance_id,
            failure_mode,
            deployment_id,
            active,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            thread_id,
            agent_id,
            instance_id,
            failure_mode: failure_mode.into(),
            deployment_id,
            active,
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
        "SELECT l.thread_id,l.agent_id,l.instance_id,d.failure_mode,n.deployment_id AS deployment_id, EXISTS(SELECT 1 FROM chat_invocations i WHERE i.thread_id=l.thread_id AND i.agent_id=l.agent_id AND i.status IN ('pending','running')) AS active FROM thread_leases l JOIN agents a ON a.id=l.agent_id JOIN agent_deployment_settings d ON d.agent_id=l.agent_id LEFT JOIN agent_instances n ON n.agent_id=l.agent_id AND n.instance_id=l.instance_id WHERE a.deleted_at IS NULL AND (n.instance_id IS NULL OR NOT n.ready OR n.last_seen_at<NOW()-make_interval(secs=>$1::float8)) ORDER BY l.updated_at LIMIT 200",
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
        p1: &'a f64,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 1> {
        RecordQuery {
            client,
            params: [p1],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        thread_id: row.try_get(0)?,
                        agent_id: row.try_get(1)?,
                        instance_id: row.try_get(2)?,
                        failure_mode: row.try_get(3)?,
                        deployment_id: row.try_get(4)?,
                        active: row.try_get(5)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
