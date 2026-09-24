// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct SessionsParams<T1: crate::StringSql> {
    pub user_id: Option<uuid::Uuid>,
    pub agent: uuid::Uuid,
    pub after: Option<uuid::Uuid>,
    pub query: T1,
    pub limit: i64,
}
#[derive(Clone, Copy, Debug)]
pub struct ReadStateParams {
    pub thread: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub sequence: i64,
    pub unread: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct UnreadParams {
    pub thread: uuid::Uuid,
    pub user_id: uuid::Uuid,
}
#[derive(Debug)]
pub struct RenameParams<T1: crate::StringSql> {
    pub title: T1,
    pub thread: uuid::Uuid,
}
#[derive(Clone, Copy, Debug)]
pub struct LatestRunParams {
    pub thread: uuid::Uuid,
    pub agent: uuid::Uuid,
}
#[derive(Clone, Copy, Debug)]
pub struct QueueParams {
    pub thread: uuid::Uuid,
    pub agent: uuid::Uuid,
}
#[derive(Clone, Copy, Debug)]
pub struct RemoveInputParams {
    pub invocation: uuid::Uuid,
    pub id: uuid::Uuid,
}
#[derive(Clone, Copy, Debug)]
pub struct OrderInputParams {
    pub position: i64,
    pub invocation: uuid::Uuid,
    pub id: uuid::Uuid,
}
#[derive(Clone, Copy, Debug)]
pub struct SidecarQueueParams {
    pub thread: uuid::Uuid,
    pub agent: uuid::Uuid,
}
#[derive(Clone, Copy, Debug)]
pub struct ClearSidecarQueueParams {
    pub thread: uuid::Uuid,
    pub agent: uuid::Uuid,
}
#[derive(Debug)]
pub struct PutSidecarQueueParams<T1: crate::StringSql, T2: crate::StringSql> {
    pub thread: uuid::Uuid,
    pub agent: uuid::Uuid,
    pub id: uuid::Uuid,
    pub invocation: uuid::Uuid,
    pub text: T1,
    pub history: T2,
    pub position: i64,
}
#[derive(Debug)]
pub struct SearchMessagesParams<T1: crate::StringSql> {
    pub thread: uuid::Uuid,
    pub query: T1,
    pub after: Option<uuid::Uuid>,
    pub limit: i64,
}
#[derive(Clone, Copy, Debug)]
pub struct CancelMessageParams {
    pub thread: uuid::Uuid,
    pub id: uuid::Uuid,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub id: uuid::Uuid,
    pub title: String,
    pub primary_agent_id: uuid::Uuid,
    pub preview: String,
    pub unread: bool,
}
pub struct SessionBorrowed<'a> {
    pub id: uuid::Uuid,
    pub title: &'a str,
    pub primary_agent_id: uuid::Uuid,
    pub preview: &'a str,
    pub unread: bool,
}
impl<'a> From<SessionBorrowed<'a>> for Session {
    fn from(
        SessionBorrowed {
            id,
            title,
            primary_agent_id,
            preview,
            unread,
        }: SessionBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            primary_agent_id,
            preview: preview.into(),
            unread,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Copy)]
pub struct ReadState {
    pub unread: bool,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Agent {
    pub id: uuid::Uuid,
    pub name: String,
}
pub struct AgentBorrowed<'a> {
    pub id: uuid::Uuid,
    pub name: &'a str,
}
impl<'a> From<AgentBorrowed<'a>> for Agent {
    fn from(AgentBorrowed { id, name }: AgentBorrowed<'a>) -> Self {
        Self {
            id,
            name: name.into(),
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub id: uuid::Uuid,
    pub status: String,
    pub invocation_id: uuid::Uuid,
    pub invocation_status: String,
}
pub struct RunBorrowed<'a> {
    pub id: uuid::Uuid,
    pub status: &'a str,
    pub invocation_id: uuid::Uuid,
    pub invocation_status: &'a str,
}
impl<'a> From<RunBorrowed<'a>> for Run {
    fn from(
        RunBorrowed {
            id,
            status,
            invocation_id,
            invocation_status,
        }: RunBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            status: status.into(),
            invocation_id,
            invocation_status: invocation_status.into(),
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct Queue {
    pub id: uuid::Uuid,
    pub invocation_id: uuid::Uuid,
    pub text: String,
    pub history_through_message_id: String,
    pub position: i64,
}
pub struct QueueBorrowed<'a> {
    pub id: uuid::Uuid,
    pub invocation_id: uuid::Uuid,
    pub text: &'a str,
    pub history_through_message_id: &'a str,
    pub position: i64,
}
impl<'a> From<QueueBorrowed<'a>> for Queue {
    fn from(
        QueueBorrowed {
            id,
            invocation_id,
            text,
            history_through_message_id,
            position,
        }: QueueBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            invocation_id,
            text: text.into(),
            history_through_message_id: history_through_message_id.into(),
            position,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Copy)]
pub struct Sequence {
    pub activity_sequence: i64,
}
#[derive(Debug, Clone, PartialEq, Copy)]
pub struct MessageId {
    pub id: uuid::Uuid,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct SessionQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<SessionBorrowed, tokio_postgres::Error>,
    mapper: fn(SessionBorrowed) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> SessionQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(self, mapper: fn(SessionBorrowed) -> R) -> SessionQuery<'c, 'a, 's, C, R, N> {
        SessionQuery {
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
pub struct ReadStateQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<ReadState, tokio_postgres::Error>,
    mapper: fn(ReadState) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> ReadStateQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(self, mapper: fn(ReadState) -> R) -> ReadStateQuery<'c, 'a, 's, C, R, N> {
        ReadStateQuery {
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
pub struct AgentQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<AgentBorrowed, tokio_postgres::Error>,
    mapper: fn(AgentBorrowed) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> AgentQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(self, mapper: fn(AgentBorrowed) -> R) -> AgentQuery<'c, 'a, 's, C, R, N> {
        AgentQuery {
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
pub struct RunQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<RunBorrowed, tokio_postgres::Error>,
    mapper: fn(RunBorrowed) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> RunQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(self, mapper: fn(RunBorrowed) -> R) -> RunQuery<'c, 'a, 's, C, R, N> {
        RunQuery {
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
pub struct QueueQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<QueueBorrowed, tokio_postgres::Error>,
    mapper: fn(QueueBorrowed) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> QueueQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(self, mapper: fn(QueueBorrowed) -> R) -> QueueQuery<'c, 'a, 's, C, R, N> {
        QueueQuery {
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
pub struct SequenceQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<Sequence, tokio_postgres::Error>,
    mapper: fn(Sequence) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> SequenceQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(self, mapper: fn(Sequence) -> R) -> SequenceQuery<'c, 'a, 's, C, R, N> {
        SequenceQuery {
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
pub struct MessageIdQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<MessageId, tokio_postgres::Error>,
    mapper: fn(MessageId) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> MessageIdQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(self, mapper: fn(MessageId) -> R) -> MessageIdQuery<'c, 'a, 's, C, R, N> {
        MessageIdQuery {
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
pub struct AttestIdentityStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn attest_identity() -> AttestIdentityStmt {
    AttestIdentityStmt(
        "UPDATE chat_users SET attested_at=NOW() WHERE id=$1 AND attested_at IS NULL",
        None,
    )
}
impl AttestIdentityStmt {
    pub async fn prepare<'a, C: GenericClient>(
        mut self,
        client: &'a C,
    ) -> Result<Self, tokio_postgres::Error> {
        self.1 = Some(client.prepare(self.0).await?);
        Ok(self)
    }
    pub async fn bind<'c, 'a, 's, C: GenericClient>(
        &'s self,
        client: &'c C,
        user_id: &'a uuid::Uuid,
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[user_id]).await
    }
}
pub struct SessionsStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn sessions() -> SessionsStmt {
    SessionsStmt(
        "SELECT t.id,t.title,t.primary_agent_id, COALESCE((SELECT m.text FROM chat_messages m WHERE m.thread_id=t.id ORDER BY m.created_at DESC,m.id DESC LIMIT 1),'') AS preview, (COALESCE(r.unread,FALSE) OR COALESCE(r.sequence,0)<COALESCE((SELECT MAX(a.sequence) FROM chat_activity a WHERE a.thread_id=t.id AND a.kind IN ('message.created','message.completed','message.delta')),0)) AS unread FROM chat_threads t LEFT JOIN tilde_chat_reads r ON r.thread_id=t.id AND r.user_id=$1 WHERE EXISTS(SELECT 1 FROM chat_participants p WHERE p.thread_id=t.id AND p.agent_id=$2 AND p.active) AND ($1::UUID IS NULL OR EXISTS(SELECT 1 FROM chat_participants p WHERE p.thread_id=t.id AND p.user_id=$1 AND p.active)) AND NOT EXISTS(SELECT 1 FROM chat_channel_threads c WHERE c.thread_id=t.id) AND ($3::UUID IS NULL OR t.id>$3) AND ($4='' OR t.title ILIKE '%' || $4 || '%' OR EXISTS(SELECT 1 FROM chat_messages m WHERE m.thread_id=t.id AND m.text ILIKE '%' || $4 || '%')) ORDER BY t.id LIMIT $5",
        None,
    )
}
impl SessionsStmt {
    pub async fn prepare<'a, C: GenericClient>(
        mut self,
        client: &'a C,
    ) -> Result<Self, tokio_postgres::Error> {
        self.1 = Some(client.prepare(self.0).await?);
        Ok(self)
    }
    pub fn bind<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>(
        &'s self,
        client: &'c C,
        user_id: &'a Option<uuid::Uuid>,
        agent: &'a uuid::Uuid,
        after: &'a Option<uuid::Uuid>,
        query: &'a T1,
        limit: &'a i64,
    ) -> SessionQuery<'c, 'a, 's, C, Session, 5> {
        SessionQuery {
            client,
            params: [user_id, agent, after, query, limit],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<SessionBorrowed, tokio_postgres::Error> {
                    Ok(SessionBorrowed {
                        id: row.try_get(0)?,
                        title: row.try_get(1)?,
                        primary_agent_id: row.try_get(2)?,
                        preview: row.try_get(3)?,
                        unread: row.try_get(4)?,
                    })
                },
            mapper: |it| Session::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        SessionsParams<T1>,
        SessionQuery<'c, 'a, 's, C, Session, 5>,
        C,
    > for SessionsStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a SessionsParams<T1>,
    ) -> SessionQuery<'c, 'a, 's, C, Session, 5> {
        self.bind(
            client,
            &params.user_id,
            &params.agent,
            &params.after,
            &params.query,
            &params.limit,
        )
    }
}
pub struct ReadStateStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn read_state() -> ReadStateStmt {
    ReadStateStmt(
        "INSERT INTO tilde_chat_reads(thread_id,user_id,sequence,unread) VALUES($1,$2,$3,$4) ON CONFLICT(thread_id,user_id) DO UPDATE SET sequence=GREATEST(tilde_chat_reads.sequence,EXCLUDED.sequence),unread=EXCLUDED.unread",
        None,
    )
}
impl ReadStateStmt {
    pub async fn prepare<'a, C: GenericClient>(
        mut self,
        client: &'a C,
    ) -> Result<Self, tokio_postgres::Error> {
        self.1 = Some(client.prepare(self.0).await?);
        Ok(self)
    }
    pub async fn bind<'c, 'a, 's, C: GenericClient>(
        &'s self,
        client: &'c C,
        thread: &'a uuid::Uuid,
        user_id: &'a uuid::Uuid,
        sequence: &'a i64,
        unread: &'a bool,
    ) -> Result<u64, tokio_postgres::Error> {
        client
            .execute(self.0, &[thread, user_id, sequence, unread])
            .await
    }
}
impl<'a, C: GenericClient + Send + Sync>
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        ReadStateParams,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for ReadStateStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a ReadStateParams,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(
            client,
            &params.thread,
            &params.user_id,
            &params.sequence,
            &params.unread,
        ))
    }
}
pub struct UnreadStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn unread() -> UnreadStmt {
    UnreadStmt(
        "SELECT (COALESCE(r.unread,FALSE) OR COALESCE(r.sequence,0)<COALESCE((SELECT MAX(a.sequence) FROM chat_activity a WHERE a.thread_id=$1 AND a.kind IN ('message.created','message.completed','message.delta')),0)) AS unread FROM chat_threads t LEFT JOIN tilde_chat_reads r ON r.thread_id=t.id AND r.user_id=$2 WHERE t.id=$1",
        None,
    )
}
impl UnreadStmt {
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
        thread: &'a uuid::Uuid,
        user_id: &'a uuid::Uuid,
    ) -> ReadStateQuery<'c, 'a, 's, C, ReadState, 2> {
        ReadStateQuery {
            client,
            params: [thread, user_id],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row: &tokio_postgres::Row| -> Result<ReadState, tokio_postgres::Error> {
                Ok(ReadState {
                    unread: row.try_get(0)?,
                })
            },
            mapper: |it| ReadState::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        UnreadParams,
        ReadStateQuery<'c, 'a, 's, C, ReadState, 2>,
        C,
    > for UnreadStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a UnreadParams,
    ) -> ReadStateQuery<'c, 'a, 's, C, ReadState, 2> {
        self.bind(client, &params.thread, &params.user_id)
    }
}
pub struct RenameStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn rename() -> RenameStmt {
    RenameStmt("UPDATE chat_threads SET title=$1 WHERE id=$2", None)
}
impl RenameStmt {
    pub async fn prepare<'a, C: GenericClient>(
        mut self,
        client: &'a C,
    ) -> Result<Self, tokio_postgres::Error> {
        self.1 = Some(client.prepare(self.0).await?);
        Ok(self)
    }
    pub async fn bind<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>(
        &'s self,
        client: &'c C,
        title: &'a T1,
        thread: &'a uuid::Uuid,
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[title, thread]).await
    }
}
impl<'a, C: GenericClient + Send + Sync, T1: crate::StringSql>
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        RenameParams<T1>,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for RenameStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a RenameParams<T1>,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(client, &params.title, &params.thread))
    }
}
pub struct AgentStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn agent() -> AgentStmt {
    AgentStmt(
        "SELECT id,name FROM agents WHERE id=$1 AND deleted_at IS NULL",
        None,
    )
}
impl AgentStmt {
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
        agent: &'a uuid::Uuid,
    ) -> AgentQuery<'c, 'a, 's, C, Agent, 1> {
        AgentQuery {
            client,
            params: [agent],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row: &tokio_postgres::Row| -> Result<AgentBorrowed, tokio_postgres::Error> {
                Ok(AgentBorrowed {
                    id: row.try_get(0)?,
                    name: row.try_get(1)?,
                })
            },
            mapper: |it| Agent::from(it),
        }
    }
}
pub struct LatestRunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn latest_run() -> LatestRunStmt {
    LatestRunStmt(
        "SELECT r.id,r.status,v.id AS invocation_id,v.status AS invocation_status FROM chat_runs r JOIN chat_invocations v ON v.run_id=r.id WHERE r.thread_id=$1 AND r.agent_id=$2 ORDER BY v.started_at DESC NULLS FIRST,v.id DESC LIMIT 1",
        None,
    )
}
impl LatestRunStmt {
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
        thread: &'a uuid::Uuid,
        agent: &'a uuid::Uuid,
    ) -> RunQuery<'c, 'a, 's, C, Run, 2> {
        RunQuery {
            client,
            params: [thread, agent],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row: &tokio_postgres::Row| -> Result<RunBorrowed, tokio_postgres::Error> {
                Ok(RunBorrowed {
                    id: row.try_get(0)?,
                    status: row.try_get(1)?,
                    invocation_id: row.try_get(2)?,
                    invocation_status: row.try_get(3)?,
                })
            },
            mapper: |it| Run::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient>
    crate::client::async_::Params<'c, 'a, 's, LatestRunParams, RunQuery<'c, 'a, 's, C, Run, 2>, C>
    for LatestRunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a LatestRunParams,
    ) -> RunQuery<'c, 'a, 's, C, Run, 2> {
        self.bind(client, &params.thread, &params.agent)
    }
}
pub struct QueueStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn queue() -> QueueStmt {
    QueueStmt(
        "SELECT i.id,i.invocation_id,i.text,COALESCE(i.history_through_message_id::TEXT,'') AS history_through_message_id,i.queue_order AS position FROM chat_inputs i JOIN chat_invocations v ON v.id=i.invocation_id WHERE v.thread_id=$1 AND v.agent_id=$2 AND NOT i.accepted ORDER BY i.queue_order,i.sequence",
        None,
    )
}
impl QueueStmt {
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
        thread: &'a uuid::Uuid,
        agent: &'a uuid::Uuid,
    ) -> QueueQuery<'c, 'a, 's, C, Queue, 2> {
        QueueQuery {
            client,
            params: [thread, agent],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row: &tokio_postgres::Row| -> Result<QueueBorrowed, tokio_postgres::Error> {
                Ok(QueueBorrowed {
                    id: row.try_get(0)?,
                    invocation_id: row.try_get(1)?,
                    text: row.try_get(2)?,
                    history_through_message_id: row.try_get(3)?,
                    position: row.try_get(4)?,
                })
            },
            mapper: |it| Queue::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient>
    crate::client::async_::Params<'c, 'a, 's, QueueParams, QueueQuery<'c, 'a, 's, C, Queue, 2>, C>
    for QueueStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a QueueParams,
    ) -> QueueQuery<'c, 'a, 's, C, Queue, 2> {
        self.bind(client, &params.thread, &params.agent)
    }
}
pub struct RemoveInputStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn remove_input() -> RemoveInputStmt {
    RemoveInputStmt(
        "UPDATE chat_inputs SET accepted=TRUE WHERE invocation_id=$1 AND id=$2 AND NOT accepted",
        None,
    )
}
impl RemoveInputStmt {
    pub async fn prepare<'a, C: GenericClient>(
        mut self,
        client: &'a C,
    ) -> Result<Self, tokio_postgres::Error> {
        self.1 = Some(client.prepare(self.0).await?);
        Ok(self)
    }
    pub async fn bind<'c, 'a, 's, C: GenericClient>(
        &'s self,
        client: &'c C,
        invocation: &'a uuid::Uuid,
        id: &'a uuid::Uuid,
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[invocation, id]).await
    }
}
impl<'a, C: GenericClient + Send + Sync>
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        RemoveInputParams,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for RemoveInputStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a RemoveInputParams,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(client, &params.invocation, &params.id))
    }
}
pub struct OrderInputStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn order_input() -> OrderInputStmt {
    OrderInputStmt(
        "UPDATE chat_inputs SET queue_order=$1 WHERE invocation_id=$2 AND id=$3 AND NOT accepted",
        None,
    )
}
impl OrderInputStmt {
    pub async fn prepare<'a, C: GenericClient>(
        mut self,
        client: &'a C,
    ) -> Result<Self, tokio_postgres::Error> {
        self.1 = Some(client.prepare(self.0).await?);
        Ok(self)
    }
    pub async fn bind<'c, 'a, 's, C: GenericClient>(
        &'s self,
        client: &'c C,
        position: &'a i64,
        invocation: &'a uuid::Uuid,
        id: &'a uuid::Uuid,
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[position, invocation, id]).await
    }
}
impl<'a, C: GenericClient + Send + Sync>
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        OrderInputParams,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for OrderInputStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a OrderInputParams,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(client, &params.position, &params.invocation, &params.id))
    }
}
pub struct SidecarQueueStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn sidecar_queue() -> SidecarQueueStmt {
    SidecarQueueStmt(
        "SELECT id,invocation_id,text,history_through_message_id,position FROM tilde_chat_sidecar_queue WHERE thread_id=$1 AND agent_id=$2 ORDER BY position",
        None,
    )
}
impl SidecarQueueStmt {
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
        thread: &'a uuid::Uuid,
        agent: &'a uuid::Uuid,
    ) -> QueueQuery<'c, 'a, 's, C, Queue, 2> {
        QueueQuery {
            client,
            params: [thread, agent],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row: &tokio_postgres::Row| -> Result<QueueBorrowed, tokio_postgres::Error> {
                Ok(QueueBorrowed {
                    id: row.try_get(0)?,
                    invocation_id: row.try_get(1)?,
                    text: row.try_get(2)?,
                    history_through_message_id: row.try_get(3)?,
                    position: row.try_get(4)?,
                })
            },
            mapper: |it| Queue::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        SidecarQueueParams,
        QueueQuery<'c, 'a, 's, C, Queue, 2>,
        C,
    > for SidecarQueueStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a SidecarQueueParams,
    ) -> QueueQuery<'c, 'a, 's, C, Queue, 2> {
        self.bind(client, &params.thread, &params.agent)
    }
}
pub struct ClearSidecarQueueStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn clear_sidecar_queue() -> ClearSidecarQueueStmt {
    ClearSidecarQueueStmt(
        "DELETE FROM tilde_chat_sidecar_queue WHERE thread_id=$1 AND agent_id=$2",
        None,
    )
}
impl ClearSidecarQueueStmt {
    pub async fn prepare<'a, C: GenericClient>(
        mut self,
        client: &'a C,
    ) -> Result<Self, tokio_postgres::Error> {
        self.1 = Some(client.prepare(self.0).await?);
        Ok(self)
    }
    pub async fn bind<'c, 'a, 's, C: GenericClient>(
        &'s self,
        client: &'c C,
        thread: &'a uuid::Uuid,
        agent: &'a uuid::Uuid,
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[thread, agent]).await
    }
}
impl<'a, C: GenericClient + Send + Sync>
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        ClearSidecarQueueParams,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for ClearSidecarQueueStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a ClearSidecarQueueParams,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(client, &params.thread, &params.agent))
    }
}
pub struct PutSidecarQueueStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn put_sidecar_queue() -> PutSidecarQueueStmt {
    PutSidecarQueueStmt(
        "INSERT INTO tilde_chat_sidecar_queue(thread_id,agent_id,id,invocation_id,text,history_through_message_id,position) VALUES($1,$2,$3,$4,$5,$6,$7)",
        None,
    )
}
impl PutSidecarQueueStmt {
    pub async fn prepare<'a, C: GenericClient>(
        mut self,
        client: &'a C,
    ) -> Result<Self, tokio_postgres::Error> {
        self.1 = Some(client.prepare(self.0).await?);
        Ok(self)
    }
    pub async fn bind<'c, 'a, 's, C: GenericClient, T1: crate::StringSql, T2: crate::StringSql>(
        &'s self,
        client: &'c C,
        thread: &'a uuid::Uuid,
        agent: &'a uuid::Uuid,
        id: &'a uuid::Uuid,
        invocation: &'a uuid::Uuid,
        text: &'a T1,
        history: &'a T2,
        position: &'a i64,
    ) -> Result<u64, tokio_postgres::Error> {
        client
            .execute(
                self.0,
                &[thread, agent, id, invocation, text, history, position],
            )
            .await
    }
}
impl<'a, C: GenericClient + Send + Sync, T1: crate::StringSql, T2: crate::StringSql>
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        PutSidecarQueueParams<T1, T2>,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for PutSidecarQueueStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a PutSidecarQueueParams<T1, T2>,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(
            client,
            &params.thread,
            &params.agent,
            &params.id,
            &params.invocation,
            &params.text,
            &params.history,
            &params.position,
        ))
    }
}
pub struct SequenceStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn sequence() -> SequenceStmt {
    SequenceStmt(
        "SELECT activity_sequence FROM chat_threads WHERE id=$1",
        None,
    )
}
impl SequenceStmt {
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
        thread: &'a uuid::Uuid,
    ) -> SequenceQuery<'c, 'a, 's, C, Sequence, 1> {
        SequenceQuery {
            client,
            params: [thread],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row: &tokio_postgres::Row| -> Result<Sequence, tokio_postgres::Error> {
                Ok(Sequence {
                    activity_sequence: row.try_get(0)?,
                })
            },
            mapper: |it| Sequence::from(it),
        }
    }
}
pub struct SearchMessagesStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn search_messages() -> SearchMessagesStmt {
    SearchMessagesStmt(
        "SELECT id FROM chat_messages WHERE thread_id=$1 AND text ILIKE '%' || $2 || '%' AND ($3::UUID IS NULL OR id>$3) ORDER BY id LIMIT $4",
        None,
    )
}
impl SearchMessagesStmt {
    pub async fn prepare<'a, C: GenericClient>(
        mut self,
        client: &'a C,
    ) -> Result<Self, tokio_postgres::Error> {
        self.1 = Some(client.prepare(self.0).await?);
        Ok(self)
    }
    pub fn bind<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>(
        &'s self,
        client: &'c C,
        thread: &'a uuid::Uuid,
        query: &'a T1,
        after: &'a Option<uuid::Uuid>,
        limit: &'a i64,
    ) -> MessageIdQuery<'c, 'a, 's, C, MessageId, 4> {
        MessageIdQuery {
            client,
            params: [thread, query, after, limit],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row: &tokio_postgres::Row| -> Result<MessageId, tokio_postgres::Error> {
                Ok(MessageId {
                    id: row.try_get(0)?,
                })
            },
            mapper: |it| MessageId::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        SearchMessagesParams<T1>,
        MessageIdQuery<'c, 'a, 's, C, MessageId, 4>,
        C,
    > for SearchMessagesStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a SearchMessagesParams<T1>,
    ) -> MessageIdQuery<'c, 'a, 's, C, MessageId, 4> {
        self.bind(
            client,
            &params.thread,
            &params.query,
            &params.after,
            &params.limit,
        )
    }
}
pub struct CancelMessageStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn cancel_message() -> CancelMessageStmt {
    CancelMessageStmt(
        "UPDATE chat_messages m SET status='aborted' WHERE m.thread_id=$1 AND m.id=$2 AND EXISTS(SELECT 1 FROM chat_participants p WHERE p.id=m.participant_id AND p.user_id IS NOT NULL)",
        None,
    )
}
impl CancelMessageStmt {
    pub async fn prepare<'a, C: GenericClient>(
        mut self,
        client: &'a C,
    ) -> Result<Self, tokio_postgres::Error> {
        self.1 = Some(client.prepare(self.0).await?);
        Ok(self)
    }
    pub async fn bind<'c, 'a, 's, C: GenericClient>(
        &'s self,
        client: &'c C,
        thread: &'a uuid::Uuid,
        id: &'a uuid::Uuid,
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[thread, id]).await
    }
}
impl<'a, C: GenericClient + Send + Sync>
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        CancelMessageParams,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for CancelMessageStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a CancelMessageParams,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(client, &params.thread, &params.id))
    }
}
