// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub kind: String,
    pub name: String,
    pub input_json: String,
    pub agent_id: Option<uuid::Uuid>,
    pub thread_id: Option<uuid::Uuid>,
    pub connection_id: Option<uuid::Uuid>,
    pub credentials: Option<Vec<u8>>,
    pub connection_type: Option<String>,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub kind: &'a str,
    pub name: &'a str,
    pub input_json: &'a str,
    pub agent_id: Option<uuid::Uuid>,
    pub thread_id: Option<uuid::Uuid>,
    pub connection_id: Option<uuid::Uuid>,
    pub credentials: Option<&'a [u8]>,
    pub connection_type: Option<&'a str>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            kind,
            name,
            input_json,
            agent_id,
            thread_id,
            connection_id,
            credentials,
            connection_type,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            kind: kind.into(),
            name: name.into(),
            input_json: input_json.into(),
            agent_id,
            thread_id,
            connection_id,
            credentials: credentials.map(|v| v.into()),
            connection_type: connection_type.map(|v| v.into()),
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
        "UPDATE tool_host_calls SET status='delivered' WHERE id IN ( SELECT id FROM tool_host_calls WHERE tool_host_id=$1 AND status='pending' ORDER BY created_at FOR UPDATE SKIP LOCKED ) RETURNING id,kind,name,input_json,agent_id,thread_id,connection_id,credentials, (SELECT type_id FROM connections c WHERE c.id=tool_host_calls.connection_id) AS connection_type",
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
                        kind: row.try_get(1)?,
                        name: row.try_get(2)?,
                        input_json: row.try_get(3)?,
                        agent_id: row.try_get(4)?,
                        thread_id: row.try_get(5)?,
                        connection_id: row.try_get(6)?,
                        credentials: row.try_get(7)?,
                        connection_type: row.try_get(8)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
