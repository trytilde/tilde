// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::ArraySql<Item = uuid::Uuid>, T2: crate::ArraySql<Item = uuid::Uuid>>
{
    pub session_ids: T1,
    pub identity_ids: T2,
    pub agent_id: uuid::Uuid,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub session_id: uuid::Uuid,
    pub identity_id: uuid::Uuid,
    pub value: String,
}
pub struct RecordBorrowed<'a> {
    pub session_id: uuid::Uuid,
    pub identity_id: uuid::Uuid,
    pub value: &'a str,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            session_id,
            identity_id,
            value,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            session_id,
            identity_id,
            value: value.into(),
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
        "SELECT p.thread_id AS session_id,p.user_id AS identity_id,COALESCE(i.value,n.value,u.name) AS value FROM chat_participants p JOIN chat_users u ON u.id=p.user_id LEFT JOIN chat_channel_threads b ON b.thread_id=p.thread_id LEFT JOIN chat_channel_identities i ON i.id=u.id AND i.connection_id=b.connection_id LEFT JOIN chat_native_identities n ON n.id=u.id WHERE p.thread_id=ANY($1) AND p.user_id=ANY($2) AND (b.connection_id IS NULL OR i.id IS NOT NULL) AND EXISTS(SELECT 1 FROM chat_participants owner WHERE owner.thread_id=p.thread_id AND owner.agent_id=$3)",
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
        session_ids: &'a T1,
        identity_ids: &'a T2,
        agent_id: &'a uuid::Uuid,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 3> {
        RecordQuery {
            client,
            params: [session_ids, identity_ids, agent_id],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        session_id: row.try_get(0)?,
                        identity_id: row.try_get(1)?,
                        value: row.try_get(2)?,
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
        RecordQuery<'c, 'a, 's, C, Record, 3>,
        C,
    > for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1, T2>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 3> {
        self.bind(
            client,
            &params.session_ids,
            &params.identity_ids,
            &params.agent_id,
        )
    }
}
