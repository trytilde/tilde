// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub text: String,
    pub history_through_message_id: Option<uuid::Uuid>,
    pub source_identity_id: Option<uuid::Uuid>,
    pub channel_origin: bool,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub text: &'a str,
    pub history_through_message_id: Option<uuid::Uuid>,
    pub source_identity_id: Option<uuid::Uuid>,
    pub channel_origin: bool,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            text,
            history_through_message_id,
            source_identity_id,
            channel_origin,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            text: text.into(),
            history_through_message_id,
            source_identity_id,
            channel_origin,
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
        "SELECT i.id,i.text,i.history_through_message_id,CASE WHEN m.id IS NULL THEN r.source_identity_id ELSE m.source_identity_id END AS source_identity_id, EXISTS(SELECT 1 FROM chat_channel_threads b WHERE b.thread_id=v.thread_id) AS channel_origin FROM chat_inputs i JOIN chat_invocations v ON v.id=i.invocation_id JOIN chat_runs r ON r.id=v.run_id LEFT JOIN chat_messages m ON m.id=i.id AND m.thread_id=v.thread_id WHERE i.invocation_id=$1 AND NOT i.accepted AND (m.source_identity_id IS NULL OR chat_identity_allowed(v.agent_id,m.source_identity_id)) ORDER BY i.queue_order,i.sequence",
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
                        text: row.try_get(1)?,
                        history_through_message_id: row.try_get(2)?,
                        source_identity_id: row.try_get(3)?,
                        channel_origin: row.try_get(4)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
