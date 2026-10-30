// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub at: chrono::DateTime<chrono::Utc>,
    pub operation: String,
    pub kind: String,
    pub path: String,
    pub title: String,
    pub content: String,
    pub valid_from: Option<chrono::DateTime<chrono::Utc>>,
    pub valid_to: Option<chrono::DateTime<chrono::Utc>>,
    pub importance: f32,
    pub origin: String,
    pub identity_id: Option<uuid::Uuid>,
    pub agent_id: Option<uuid::Uuid>,
}
pub struct RecordBorrowed<'a> {
    pub at: chrono::DateTime<chrono::Utc>,
    pub operation: &'a str,
    pub kind: &'a str,
    pub path: &'a str,
    pub title: &'a str,
    pub content: &'a str,
    pub valid_from: Option<chrono::DateTime<chrono::Utc>>,
    pub valid_to: Option<chrono::DateTime<chrono::Utc>>,
    pub importance: f32,
    pub origin: &'a str,
    pub identity_id: Option<uuid::Uuid>,
    pub agent_id: Option<uuid::Uuid>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            at,
            operation,
            kind,
            path,
            title,
            content,
            valid_from,
            valid_to,
            importance,
            origin,
            identity_id,
            agent_id,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            at,
            operation: operation.into(),
            kind: kind.into(),
            path: path.into(),
            title: title.into(),
            content: content.into(),
            valid_from,
            valid_to,
            importance,
            origin: origin.into(),
            identity_id,
            agent_id,
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
        "SELECT at,operation,kind,path,title,content,lower(valid_during) AS valid_from,upper(valid_during) AS valid_to, importance,origin,identity_id,agent_id FROM memory_revisions WHERE memory_id=$1 ORDER BY id",
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
                        at: row.try_get(0)?,
                        operation: row.try_get(1)?,
                        kind: row.try_get(2)?,
                        path: row.try_get(3)?,
                        title: row.try_get(4)?,
                        content: row.try_get(5)?,
                        valid_from: row.try_get(6)?,
                        valid_to: row.try_get(7)?,
                        importance: row.try_get(8)?,
                        origin: row.try_get(9)?,
                        identity_id: row.try_get(10)?,
                        agent_id: row.try_get(11)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
