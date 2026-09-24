// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub connection_id: Option<uuid::Uuid>,
    pub external_message_id: Option<String>,
    pub destination: Option<String>,
    pub delivery_status: Option<String>,
    pub id: uuid::Uuid,
    pub thread_id: uuid::Uuid,
    pub participant_id: uuid::Uuid,
    pub text: String,
    pub status: String,
    pub in_reply_to_message_id: Option<uuid::Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub format: String,
    pub subject: Option<String>,
    pub invocation_id: Option<uuid::Uuid>,
    pub attachments: serde_json::Value,
    pub targets: Vec<uuid::Uuid>,
}
pub struct RecordBorrowed<'a> {
    pub connection_id: Option<uuid::Uuid>,
    pub external_message_id: Option<&'a str>,
    pub destination: Option<&'a str>,
    pub delivery_status: Option<&'a str>,
    pub id: uuid::Uuid,
    pub thread_id: uuid::Uuid,
    pub participant_id: uuid::Uuid,
    pub text: &'a str,
    pub status: &'a str,
    pub in_reply_to_message_id: Option<uuid::Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub format: &'a str,
    pub subject: Option<&'a str>,
    pub invocation_id: Option<uuid::Uuid>,
    pub attachments: postgres_types::Json<&'a serde_json::value::RawValue>,
    pub targets: crate::ArrayIterator<'a, uuid::Uuid>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            connection_id,
            external_message_id,
            destination,
            delivery_status,
            id,
            thread_id,
            participant_id,
            text,
            status,
            in_reply_to_message_id,
            created_at,
            format,
            subject,
            invocation_id,
            attachments,
            targets,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            connection_id,
            external_message_id: external_message_id.map(|v| v.into()),
            destination: destination.map(|v| v.into()),
            delivery_status: delivery_status.map(|v| v.into()),
            id,
            thread_id,
            participant_id,
            text: text.into(),
            status: status.into(),
            in_reply_to_message_id,
            created_at,
            format: format.into(),
            subject: subject.map(|v| v.into()),
            invocation_id,
            attachments: serde_json::from_str(attachments.0.get()).unwrap(),
            targets: targets.map(|v| v).collect(),
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
        "SELECT d.connection_id AS connection_id,d.external_message_id AS external_message_id,d.destination AS destination,d.status AS delivery_status,m.id,m.thread_id,m.participant_id,m.text,m.status,m.in_reply_to_message_id,m.created_at,m.format,m.subject,m.invocation_id,COALESCE((SELECT jsonb_agg(jsonb_build_object('id',a.id,'thread_id',a.thread_id,'filename',a.filename,'media_type',a.media_type,'size_bytes',a.size_bytes,'sha256',a.sha256) ORDER BY a.id) FROM chat_message_attachments ma JOIN chat_attachments a ON a.id=ma.attachment_id WHERE ma.message_id=m.id),'[]'::jsonb) AS attachments,ARRAY(SELECT participant_id FROM chat_message_targets WHERE message_id=m.id ORDER BY participant_id) AS targets FROM chat_messages m LEFT JOIN chat_message_deliveries d ON d.message_id=m.id WHERE m.id=$1",
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
                        connection_id: row.try_get(0)?,
                        external_message_id: row.try_get(1)?,
                        destination: row.try_get(2)?,
                        delivery_status: row.try_get(3)?,
                        id: row.try_get(4)?,
                        thread_id: row.try_get(5)?,
                        participant_id: row.try_get(6)?,
                        text: row.try_get(7)?,
                        status: row.try_get(8)?,
                        in_reply_to_message_id: row.try_get(9)?,
                        created_at: row.try_get(10)?,
                        format: row.try_get(11)?,
                        subject: row.try_get(12)?,
                        invocation_id: row.try_get(13)?,
                        attachments: row.try_get(14)?,
                        targets: row.try_get(15)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
