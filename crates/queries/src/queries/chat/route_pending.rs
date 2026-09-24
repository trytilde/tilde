// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub channel_origin: bool,
    pub id: uuid::Uuid,
    pub thread_id: uuid::Uuid,
    pub source_identity_id: Option<uuid::Uuid>,
    pub traceparent: String,
    pub tracestate: String,
    pub text: String,
    pub agent_id: uuid::Uuid,
}
pub struct RecordBorrowed<'a> {
    pub channel_origin: bool,
    pub id: uuid::Uuid,
    pub thread_id: uuid::Uuid,
    pub source_identity_id: Option<uuid::Uuid>,
    pub traceparent: &'a str,
    pub tracestate: &'a str,
    pub text: &'a str,
    pub agent_id: uuid::Uuid,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            channel_origin,
            id,
            thread_id,
            source_identity_id,
            traceparent,
            tracestate,
            text,
            agent_id,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            channel_origin,
            id,
            thread_id,
            source_identity_id,
            traceparent: traceparent.into(),
            tracestate: tracestate.into(),
            text: text.into(),
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
        "SELECT EXISTS(SELECT 1 FROM chat_channel_threads b WHERE b.thread_id=m.thread_id) AS channel_origin,m.id,m.thread_id,m.source_identity_id,m.traceparent,m.tracestate,CASE WHEN m.text<>'' THEN m.text ELSE 'New message with attachments.' END AS text,p.agent_id AS agent_id FROM chat_messages m JOIN chat_threads r ON r.id=m.thread_id JOIN chat_participants sender ON sender.id=m.participant_id LEFT JOIN chat_messages reply ON reply.id=m.in_reply_to_message_id LEFT JOIN chat_participants reply_author ON reply_author.id=reply.participant_id JOIN chat_participants p ON p.thread_id=m.thread_id AND p.agent_id IS NOT NULL AND p.active JOIN agents a ON a.id=p.agent_id AND NOT a.paused AND a.deleted_at IS NULL WHERE NOT EXISTS(SELECT 1 FROM agent_deployments d WHERE d.id=p.deployment_id AND d.target='sidecar' AND d.status='registered') AND EXISTS (SELECT 1 FROM agent_deployments eligible WHERE eligible.agent_id=p.agent_id AND eligible.status='registered' AND (eligible.id=p.deployment_id OR NOT EXISTS(SELECT 1 FROM agent_deployments pinned WHERE pinned.id=p.deployment_id AND pinned.status='registered')) AND (eligible.id=p.deployment_id OR eligible.traffic_weight>0 OR EXISTS(SELECT 1 FROM agent_deployment_settings policy WHERE policy.agent_id=p.agent_id AND policy.routing='latest')) AND (eligible.target='lambda' OR EXISTS(SELECT 1 FROM agent_instances n WHERE n.agent_id=p.agent_id AND n.deployment_id=eligible.id AND n.connection_id IS NOT NULL AND n.ready AND n.agent_ready AND n.agent_connected AND n.last_seen_at>NOW()-make_interval(secs=>$1::float8)))) AND m.status='complete' AND chat_channel_message_allowed(p.agent_id,m.thread_id,m.source_identity_id) AND (EXISTS(SELECT 1 FROM chat_message_targets t WHERE t.message_id=m.id AND t.participant_id=p.id) OR (sender.user_id IS NOT NULL AND p.agent_id=COALESCE(reply_author.agent_id,r.primary_agent_id) AND NOT EXISTS(SELECT 1 FROM chat_message_targets t WHERE t.message_id=m.id))) AND NOT EXISTS(SELECT 1 FROM chat_message_dispatch d WHERE d.message_id=m.id AND d.agent_id=p.agent_id) ORDER BY m.created_at LIMIT 50",
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
        p1: &'a f64,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 1> {
        RecordQuery {
            client,
            params: [p1],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        channel_origin: row.try_get(0)?,
                        id: row.try_get(1)?,
                        thread_id: row.try_get(2)?,
                        source_identity_id: row.try_get(3)?,
                        traceparent: row.try_get(4)?,
                        tracestate: row.try_get(5)?,
                        text: row.try_get(6)?,
                        agent_id: row.try_get(7)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
