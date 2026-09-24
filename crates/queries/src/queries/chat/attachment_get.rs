// This file was generated with `cornucopia`. Do not modify.

#[derive(Clone, Copy, Debug)]
pub struct RunParams {
    pub p1: uuid::Uuid,
    pub p2: uuid::Uuid,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub object_key: Option<String>,
    pub id: uuid::Uuid,
    pub thread_id: uuid::Uuid,
    pub filename: String,
    pub media_type: String,
    pub size_bytes: i64,
    pub sha256: String,
    pub content: Option<Vec<u8>>,
    pub connection_id: Option<uuid::Uuid>,
    pub provider_attachment_id: Option<String>,
    pub source_url: Option<Vec<u8>>,
}
pub struct RecordBorrowed<'a> {
    pub object_key: Option<&'a str>,
    pub id: uuid::Uuid,
    pub thread_id: uuid::Uuid,
    pub filename: &'a str,
    pub media_type: &'a str,
    pub size_bytes: i64,
    pub sha256: &'a str,
    pub content: Option<&'a [u8]>,
    pub connection_id: Option<uuid::Uuid>,
    pub provider_attachment_id: Option<&'a str>,
    pub source_url: Option<&'a [u8]>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            object_key,
            id,
            thread_id,
            filename,
            media_type,
            size_bytes,
            sha256,
            content,
            connection_id,
            provider_attachment_id,
            source_url,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            object_key: object_key.map(|v| v.into()),
            id,
            thread_id,
            filename: filename.into(),
            media_type: media_type.into(),
            size_bytes,
            sha256: sha256.into(),
            content: content.map(|v| v.into()),
            connection_id,
            provider_attachment_id: provider_attachment_id.map(|v| v.into()),
            source_url: source_url.map(|v| v.into()),
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
        "SELECT object_key,id,thread_id,filename,media_type,size_bytes,sha256,content,connection_id,provider_attachment_id,source_url FROM chat_attachments WHERE id=$1 AND thread_id=$2",
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
        p2: &'a uuid::Uuid,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        RecordQuery {
            client,
            params: [p1, p2],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        object_key: row.try_get(0)?,
                        id: row.try_get(1)?,
                        thread_id: row.try_get(2)?,
                        filename: row.try_get(3)?,
                        media_type: row.try_get(4)?,
                        size_bytes: row.try_get(5)?,
                        sha256: row.try_get(6)?,
                        content: row.try_get(7)?,
                        connection_id: row.try_get(8)?,
                        provider_attachment_id: row.try_get(9)?,
                        source_url: row.try_get(10)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient>
    crate::client::async_::Params<'c, 'a, 's, RunParams, RecordQuery<'c, 'a, 's, C, Record, 2>, C>
    for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        self.bind(client, &params.p1, &params.p2)
    }
}
