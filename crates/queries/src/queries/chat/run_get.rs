// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub thread_id: uuid::Uuid,
    pub agent_id: uuid::Uuid,
    pub objective: String,
    pub status: String,
    pub goal_id: Option<uuid::Uuid>,
    pub invocation_id: uuid::Uuid,
    pub invocation_status: String,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub thread_id: uuid::Uuid,
    pub agent_id: uuid::Uuid,
    pub objective: &'a str,
    pub status: &'a str,
    pub goal_id: Option<uuid::Uuid>,
    pub invocation_id: uuid::Uuid,
    pub invocation_status: &'a str,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            thread_id,
            agent_id,
            objective,
            status,
            goal_id,
            invocation_id,
            invocation_status,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            thread_id,
            agent_id,
            objective: objective.into(),
            status: status.into(),
            goal_id,
            invocation_id,
            invocation_status: invocation_status.into(),
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
        "SELECT r.id,r.thread_id,r.agent_id,r.objective,r.status,r.goal_id,i.id AS invocation_id,i.status AS invocation_status FROM chat_runs r JOIN chat_invocations i ON i.run_id=r.id WHERE r.id=$1 ORDER BY i.started_at DESC NULLS FIRST LIMIT 1",
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
                        thread_id: row.try_get(1)?,
                        agent_id: row.try_get(2)?,
                        objective: row.try_get(3)?,
                        status: row.try_get(4)?,
                        goal_id: row.try_get(5)?,
                        invocation_id: row.try_get(6)?,
                        invocation_status: row.try_get(7)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
