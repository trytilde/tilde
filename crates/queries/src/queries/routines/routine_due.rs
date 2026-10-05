// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub agent_id: uuid::Uuid,
    pub name: String,
    pub prompt: String,
    pub thread_title: String,
    pub schedule: Option<String>,
    pub next_run_at: Option<chrono::DateTime<chrono::Utc>>,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub agent_id: uuid::Uuid,
    pub name: &'a str,
    pub prompt: &'a str,
    pub thread_title: &'a str,
    pub schedule: Option<&'a str>,
    pub next_run_at: Option<chrono::DateTime<chrono::Utc>>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            agent_id,
            name,
            prompt,
            thread_title,
            schedule,
            next_run_at,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            agent_id,
            name: name.into(),
            prompt: prompt.into(),
            thread_title: thread_title.into(),
            schedule: schedule.map(|v| v.into()),
            next_run_at,
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
        "SELECT r.id,r.agent_id,r.name,r.prompt,r.thread_title,r.schedule,r.next_run_at FROM routines r JOIN agents a ON a.id=r.agent_id AND a.deleted_at IS NULL WHERE r.next_run_at<=NOW() ORDER BY r.next_run_at LIMIT $1 FOR UPDATE OF r SKIP LOCKED",
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
        limit: &'a i64,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 1> {
        RecordQuery {
            client,
            params: [limit],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        id: row.try_get(0)?,
                        agent_id: row.try_get(1)?,
                        name: row.try_get(2)?,
                        prompt: row.try_get(3)?,
                        thread_title: row.try_get(4)?,
                        schedule: row.try_get(5)?,
                        next_run_at: row.try_get(6)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
