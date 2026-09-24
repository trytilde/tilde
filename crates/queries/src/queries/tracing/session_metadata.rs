// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::ArraySql<Item = uuid::Uuid>, T2: crate::ArraySql<Item = uuid::Uuid>>
{
    pub agent_ids: T1,
    pub session_ids: T2,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub agent_id: uuid::Uuid,
    pub session_id: uuid::Uuid,
    pub captured_at: chrono::DateTime<chrono::Utc>,
    pub provider_id: String,
    pub provider_name: String,
    pub provider_icon_url: String,
    pub connection_id: Option<uuid::Uuid>,
    pub identity_ids: Vec<uuid::Uuid>,
    pub message_count: i64,
    pub last_turn_time: Option<chrono::DateTime<chrono::Utc>>,
}
pub struct RecordBorrowed<'a> {
    pub agent_id: uuid::Uuid,
    pub session_id: uuid::Uuid,
    pub captured_at: chrono::DateTime<chrono::Utc>,
    pub provider_id: &'a str,
    pub provider_name: &'a str,
    pub provider_icon_url: &'a str,
    pub connection_id: Option<uuid::Uuid>,
    pub identity_ids: crate::ArrayIterator<'a, uuid::Uuid>,
    pub message_count: i64,
    pub last_turn_time: Option<chrono::DateTime<chrono::Utc>>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            agent_id,
            session_id,
            captured_at,
            provider_id,
            provider_name,
            provider_icon_url,
            connection_id,
            identity_ids,
            message_count,
            last_turn_time,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            agent_id,
            session_id,
            captured_at,
            provider_id: provider_id.into(),
            provider_name: provider_name.into(),
            provider_icon_url: provider_icon_url.into(),
            connection_id,
            identity_ids: identity_ids.map(|v| v).collect(),
            message_count,
            last_turn_time,
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
        "WITH requested AS (SELECT DISTINCT * FROM UNNEST($1::UUID[],$2::UUID[]) AS r(agent_id,session_id)) SELECT r.agent_id,t.id AS session_id,statement_timestamp() AS captured_at, COALESCE(c.provider_id,'tilde') AS provider_id, COALESCE(cp.name,CASE WHEN b.connection_id IS NULL THEN 'Tilde' ELSE c.provider_id END) AS provider_name, COALESCE(cp.icon_url,'') AS provider_icon_url,b.connection_id, ARRAY(SELECT DISTINCT p.user_id FROM chat_participants p LEFT JOIN chat_channel_identities i ON i.id=p.user_id AND i.connection_id=b.connection_id WHERE p.thread_id=t.id AND p.user_id IS NOT NULL AND (b.connection_id IS NULL OR i.id IS NOT NULL) ORDER BY 1) AS identity_ids, m.message_count,GREATEST(m.last_message_time,inv.last_turn_time) AS last_turn_time FROM requested r JOIN chat_threads t ON t.id=r.session_id LEFT JOIN chat_channel_threads b ON b.thread_id=t.id LEFT JOIN connections c ON c.id=b.connection_id LEFT JOIN connection_providers cp ON cp.provider_id=c.provider_id LEFT JOIN LATERAL ( SELECT COUNT(*)::BIGINT AS message_count,MAX(created_at) AS last_message_time FROM chat_messages m WHERE m.thread_id=t.id AND m.status<>'deleted' AND (length(m.text)>0 OR EXISTS(SELECT 1 FROM chat_message_attachments a WHERE a.message_id=m.id)) ) m ON TRUE LEFT JOIN LATERAL (SELECT MAX(started_at) AS last_turn_time FROM chat_invocations i WHERE i.thread_id=t.id) inv ON TRUE WHERE EXISTS(SELECT 1 FROM chat_participants p WHERE p.thread_id=t.id AND p.agent_id=r.agent_id)",
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
    pub fn bind<
        'c,
        'a,
        's,
        C: GenericClient,
        T1: crate::ArraySql<Item = uuid::Uuid>,
        T2: crate::ArraySql<Item = uuid::Uuid>,
    >(
        &'s self,
        client: &'c C,
        agent_ids: &'a T1,
        session_ids: &'a T2,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        RecordQuery {
            client,
            params: [agent_ids, session_ids],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        agent_id: row.try_get(0)?,
                        session_id: row.try_get(1)?,
                        captured_at: row.try_get(2)?,
                        provider_id: row.try_get(3)?,
                        provider_name: row.try_get(4)?,
                        provider_icon_url: row.try_get(5)?,
                        connection_id: row.try_get(6)?,
                        identity_ids: row.try_get(7)?,
                        message_count: row.try_get(8)?,
                        last_turn_time: row.try_get(9)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
impl<
    'c,
    'a,
    's,
    C: GenericClient,
    T1: crate::ArraySql<Item = uuid::Uuid>,
    T2: crate::ArraySql<Item = uuid::Uuid>,
>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        RunParams<T1, T2>,
        RecordQuery<'c, 'a, 's, C, Record, 2>,
        C,
    > for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1, T2>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        self.bind(client, &params.agent_ids, &params.session_ids)
    }
}
