// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::ArraySql<Item = uuid::Uuid>> {
    pub p2: Option<i32>,
    pub p1: T1,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub bank_id: uuid::Uuid,
    pub bank_name: String,
    pub scope: String,
    pub kind: String,
    pub path: String,
    pub title: String,
    pub content: String,
    pub valid_from: Option<chrono::DateTime<chrono::Utc>>,
    pub valid_to: Option<chrono::DateTime<chrono::Utc>>,
    pub importance: f32,
    pub origin: String,
    pub identity_id: Option<uuid::Uuid>,
    pub written_by_agent_id: Option<uuid::Uuid>,
    pub supersedes_id: Option<uuid::Uuid>,
    pub superseded_by: Option<uuid::Uuid>,
    pub thread_id: Option<uuid::Uuid>,
    pub message_id: Option<uuid::Uuid>,
    pub embedded: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub bank_id: uuid::Uuid,
    pub bank_name: &'a str,
    pub scope: &'a str,
    pub kind: &'a str,
    pub path: &'a str,
    pub title: &'a str,
    pub content: &'a str,
    pub valid_from: Option<chrono::DateTime<chrono::Utc>>,
    pub valid_to: Option<chrono::DateTime<chrono::Utc>>,
    pub importance: f32,
    pub origin: &'a str,
    pub identity_id: Option<uuid::Uuid>,
    pub written_by_agent_id: Option<uuid::Uuid>,
    pub supersedes_id: Option<uuid::Uuid>,
    pub superseded_by: Option<uuid::Uuid>,
    pub thread_id: Option<uuid::Uuid>,
    pub message_id: Option<uuid::Uuid>,
    pub embedded: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            bank_id,
            bank_name,
            scope,
            kind,
            path,
            title,
            content,
            valid_from,
            valid_to,
            importance,
            origin,
            identity_id,
            written_by_agent_id,
            supersedes_id,
            superseded_by,
            thread_id,
            message_id,
            embedded,
            created_at,
            updated_at,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            bank_id,
            bank_name: bank_name.into(),
            scope: scope.into(),
            kind: kind.into(),
            path: path.into(),
            title: title.into(),
            content: content.into(),
            valid_from,
            valid_to,
            importance,
            origin: origin.into(),
            identity_id,
            written_by_agent_id,
            supersedes_id,
            superseded_by,
            thread_id,
            message_id,
            embedded,
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
        "SELECT m.id,m.bank_id,b.name AS bank_name,b.scope,m.kind,m.path,m.title,m.content, lower(m.valid_during) AS valid_from,upper(m.valid_during) AS valid_to,m.importance,m.origin, m.identity_id,m.written_by_agent_id,m.supersedes_id, (SELECT s.id FROM memories s WHERE s.supersedes_id=m.id) AS superseded_by,m.thread_id,m.message_id, COALESCE(m.embedding_version=$1::INTEGER,FALSE) AS embedded,m.created_at,m.updated_at FROM memories m JOIN memory_banks b ON b.id=m.bank_id WHERE m.id=ANY($2::UUID[]) AND m.deleted_at IS NULL",
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
    pub fn bind<'c, 'a, 's, C: GenericClient, T1: crate::ArraySql<Item = uuid::Uuid>>(
        &'s self,
        client: &'c C,
        p2: &'a Option<i32>,
        p1: &'a T1,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        RecordQuery {
            client,
            params: [p2, p1],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        id: row.try_get(0)?,
                        bank_id: row.try_get(1)?,
                        bank_name: row.try_get(2)?,
                        scope: row.try_get(3)?,
                        kind: row.try_get(4)?,
                        path: row.try_get(5)?,
                        title: row.try_get(6)?,
                        content: row.try_get(7)?,
                        valid_from: row.try_get(8)?,
                        valid_to: row.try_get(9)?,
                        importance: row.try_get(10)?,
                        origin: row.try_get(11)?,
                        identity_id: row.try_get(12)?,
                        written_by_agent_id: row.try_get(13)?,
                        supersedes_id: row.try_get(14)?,
                        superseded_by: row.try_get(15)?,
                        thread_id: row.try_get(16)?,
                        message_id: row.try_get(17)?,
                        embedded: row.try_get(18)?,
                        created_at: row.try_get(19)?,
                        updated_at: row.try_get(20)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::ArraySql<Item = uuid::Uuid>>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        RunParams<T1>,
        RecordQuery<'c, 'a, 's, C, Record, 2>,
        C,
    > for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        self.bind(client, &params.p2, &params.p1)
    }
}
