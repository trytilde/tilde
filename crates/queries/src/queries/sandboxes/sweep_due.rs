// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub action: String,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub action: &'a str,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(RecordBorrowed { id, action }: RecordBorrowed<'a>) -> Self {
        Self {
            id,
            action: action.into(),
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
        "SELECT id,action FROM ( SELECT s.id, CASE WHEN s.reuse <> b.reuse OR s.connection_id <> b.connection_id OR s.last_used_at < NOW() - make_interval(secs => b.terminate_after_secs) OR (s.status='failed' AND s.last_used_at < NOW() - INTERVAL '1 hour') OR NOT EXISTS (SELECT 1 FROM agent_sandboxes a WHERE a.blueprint_id=s.blueprint_id AND (s.agent_id IS NULL OR a.agent_id=s.agent_id)) THEN 'terminate' WHEN s.status='running' AND s.last_used_at < NOW() - make_interval(secs => b.sleep_after_secs) AND NOT EXISTS (SELECT 1 FROM sandbox_calls c WHERE c.sandbox_id=s.id) THEN 'sleep' END AS action FROM sandboxes s JOIN sandbox_blueprints b ON b.id=s.blueprint_id WHERE CASE WHEN $1::UUID IS NULL THEN s.lease_until IS NULL OR s.lease_until <= NOW() ELSE s.id=$1 END ) due WHERE action IS NOT NULL LIMIT 50",
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
        p1: &'a Option<uuid::Uuid>,
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
                        action: row.try_get(1)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
