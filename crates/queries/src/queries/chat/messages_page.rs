// This file was generated with `cornucopia`. Do not modify.

#[derive(Clone, Copy, Debug)]
pub struct RunParams {
    pub p3: Option<uuid::Uuid>,
    pub p4: Option<uuid::Uuid>,
    pub p2: i64,
    pub p1: uuid::Uuid,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub activity_sequence: i64,
    pub cursor_valid: bool,
    pub connection_id: Option<uuid::Uuid>,
    pub external_message_id: Option<String>,
    pub destination: Option<String>,
    pub delivery_status: Option<String>,
    pub id: Option<uuid::Uuid>,
    pub thread_id: uuid::Uuid,
    pub participant_id: Option<uuid::Uuid>,
    pub text: Option<String>,
    pub status: Option<String>,
    pub in_reply_to_message_id: Option<uuid::Uuid>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub format: Option<String>,
    pub subject: Option<String>,
    pub cached_representation: Option<serde_json::Value>,
    pub attachments: serde_json::Value,
    pub targets: Vec<uuid::Uuid>,
}
pub struct RecordBorrowed<'a> {
    pub activity_sequence: i64,
    pub cursor_valid: bool,
    pub connection_id: Option<uuid::Uuid>,
    pub external_message_id: Option<&'a str>,
    pub destination: Option<&'a str>,
    pub delivery_status: Option<&'a str>,
    pub id: Option<uuid::Uuid>,
    pub thread_id: uuid::Uuid,
    pub participant_id: Option<uuid::Uuid>,
    pub text: Option<&'a str>,
    pub status: Option<&'a str>,
    pub in_reply_to_message_id: Option<uuid::Uuid>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub format: Option<&'a str>,
    pub subject: Option<&'a str>,
    pub cached_representation: Option<postgres_types::Json<&'a serde_json::value::RawValue>>,
    pub attachments: postgres_types::Json<&'a serde_json::value::RawValue>,
    pub targets: crate::ArrayIterator<'a, uuid::Uuid>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            activity_sequence,
            cursor_valid,
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
            cached_representation,
            attachments,
            targets,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            activity_sequence,
            cursor_valid,
            connection_id,
            external_message_id: external_message_id.map(|v| v.into()),
            destination: destination.map(|v| v.into()),
            delivery_status: delivery_status.map(|v| v.into()),
            id,
            thread_id,
            participant_id,
            text: text.map(|v| v.into()),
            status: status.map(|v| v.into()),
            in_reply_to_message_id,
            created_at,
            format: format.map(|v| v.into()),
            subject: subject.map(|v| v.into()),
            cached_representation: cached_representation
                .map(|v| serde_json::from_str(v.0.get()).unwrap()),
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
        "SELECT t.activity_sequence AS activity_sequence, ($1::UUID IS NULL OR cursor.id IS NOT NULL) AS cursor_valid, d.connection_id AS connection_id, d.external_message_id AS external_message_id, d.destination AS destination, d.status AS delivery_status, m.id AS id, t.id AS thread_id, m.participant_id AS participant_id, m.text AS text, m.status AS status, m.in_reply_to_message_id, m.created_at AS created_at, m.format AS format, m.subject, cached.representation AS cached_representation, COALESCE(( SELECT jsonb_agg(jsonb_build_object( 'id', a.id, 'thread_id', a.thread_id, 'filename', a.filename, 'media_type', a.media_type, 'size_bytes', a.size_bytes, 'sha256', a.sha256 ) ORDER BY a.id) FROM chat_message_attachments ma JOIN chat_attachments a ON a.id = ma.attachment_id WHERE ma.message_id = m.id ), '[]'::jsonb) AS attachments, ARRAY(SELECT participant_id FROM chat_message_targets WHERE message_id = m.id ORDER BY participant_id) AS targets FROM chat_threads t LEFT JOIN chat_messages cursor ON cursor.id = $1 AND cursor.thread_id = t.id LEFT JOIN chat_invocations invocation ON invocation.id = $2 AND invocation.thread_id = t.id LEFT JOIN chat_messages anchor ON anchor.id = invocation.history_through_message_id LEFT JOIN LATERAL ( SELECT m.* FROM chat_messages m WHERE m.thread_id = t.id AND ($2::UUID IS NULL OR m.invocation_id = $2 OR (m.created_at, m.id) <= (anchor.created_at, anchor.id)) AND ($2::UUID IS NULL OR ( NOT (m.status='aborted' AND EXISTS(SELECT 1 FROM chat_participants p WHERE p.id=m.participant_id AND p.user_id IS NOT NULL)) AND NOT EXISTS(SELECT 1 FROM chat_inputs i JOIN chat_invocations v ON v.id=i.invocation_id WHERE i.id=m.id AND v.agent_id=invocation.agent_id AND NOT i.accepted) )) AND ($1::UUID IS NULL OR (m.created_at, m.id) < (cursor.created_at, cursor.id)) ORDER BY m.created_at DESC, m.id DESC LIMIT $3 ) m ON TRUE LEFT JOIN chat_message_deliveries d ON d.message_id = m.id LEFT JOIN chat_converted_messages cached ON cached.message_id = m.id AND cached.agent_id = invocation.agent_id WHERE t.id = $4 ORDER BY m.created_at DESC, m.id DESC",
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
        p3: &'a Option<uuid::Uuid>,
        p4: &'a Option<uuid::Uuid>,
        p2: &'a i64,
        p1: &'a uuid::Uuid,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 4> {
        RecordQuery {
            client,
            params: [p3, p4, p2, p1],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        activity_sequence: row.try_get(0)?,
                        cursor_valid: row.try_get(1)?,
                        connection_id: row.try_get(2)?,
                        external_message_id: row.try_get(3)?,
                        destination: row.try_get(4)?,
                        delivery_status: row.try_get(5)?,
                        id: row.try_get(6)?,
                        thread_id: row.try_get(7)?,
                        participant_id: row.try_get(8)?,
                        text: row.try_get(9)?,
                        status: row.try_get(10)?,
                        in_reply_to_message_id: row.try_get(11)?,
                        created_at: row.try_get(12)?,
                        format: row.try_get(13)?,
                        subject: row.try_get(14)?,
                        cached_representation: row.try_get(15)?,
                        attachments: row.try_get(16)?,
                        targets: row.try_get(17)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient>
    crate::client::async_::Params<'c, 'a, 's, RunParams, RecordQuery<'c, 'a, 's, C, Record, 4>, C>
    for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 4> {
        self.bind(client, &params.p3, &params.p4, &params.p2, &params.p1)
    }
}
