// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub thread_id: uuid::Uuid,
    pub invocation_id: uuid::Uuid,
    pub participant_id: uuid::Uuid,
    pub name: String,
    pub provider_id: String,
    pub status: String,
    pub input_json: String,
    pub output_json: String,
    pub error: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub thread_id: uuid::Uuid,
    pub invocation_id: uuid::Uuid,
    pub participant_id: uuid::Uuid,
    pub name: &'a str,
    pub provider_id: &'a str,
    pub status: &'a str,
    pub input_json: &'a str,
    pub output_json: &'a str,
    pub error: &'a str,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            thread_id,
            invocation_id,
            participant_id,
            name,
            provider_id,
            status,
            input_json,
            output_json,
            error,
            created_at,
            updated_at,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            thread_id,
            invocation_id,
            participant_id,
            name: name.into(),
            provider_id: provider_id.into(),
            status: status.into(),
            input_json: input_json.into(),
            output_json: output_json.into(),
            error: error.into(),
            created_at,
            updated_at,
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
        "SELECT t.* FROM chat_tool_calls t JOIN chat_invocations i ON i.id=t.invocation_id WHERE t.status='running' AND (i.status<>'running' OR i.lease_expires_at<=NOW()) ORDER BY t.thread_id,t.id LIMIT 100",
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
    ) -> RecordQuery<'c, 'a, 's, C, Record, 0> {
        RecordQuery {
            client,
            params: [],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        id: row.try_get(0)?,
                        thread_id: row.try_get(1)?,
                        invocation_id: row.try_get(2)?,
                        participant_id: row.try_get(3)?,
                        name: row.try_get(4)?,
                        provider_id: row.try_get(5)?,
                        status: row.try_get(6)?,
                        input_json: row.try_get(7)?,
                        output_json: row.try_get(8)?,
                        error: row.try_get(9)?,
                        created_at: row.try_get(10)?,
                        updated_at: row.try_get(11)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
