// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub memory_id: uuid::Uuid,
    pub thread_id: Option<uuid::Uuid>,
    pub agent_id: Option<uuid::Uuid>,
    pub message_id: Option<uuid::Uuid>,
    pub page_id: Option<uuid::Uuid>,
    pub revision: Option<i32>,
    pub wiki_id: Option<uuid::Uuid>,
    pub wiki_name: Option<String>,
    pub path: Option<String>,
}
pub struct RecordBorrowed<'a> {
    pub memory_id: uuid::Uuid,
    pub thread_id: Option<uuid::Uuid>,
    pub agent_id: Option<uuid::Uuid>,
    pub message_id: Option<uuid::Uuid>,
    pub page_id: Option<uuid::Uuid>,
    pub revision: Option<i32>,
    pub wiki_id: Option<uuid::Uuid>,
    pub wiki_name: Option<&'a str>,
    pub path: Option<&'a str>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            memory_id,
            thread_id,
            agent_id,
            message_id,
            page_id,
            revision,
            wiki_id,
            wiki_name,
            path,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            memory_id,
            thread_id,
            agent_id,
            message_id,
            page_id,
            revision,
            wiki_id,
            wiki_name: wiki_name.map(|v| v.into()),
            path: path.map(|v| v.into()),
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
        "SELECT e.memory_id,e.thread_id,t.primary_agent_id AS agent_id,e.message_id,e.page_id,e.revision, COALESCE(pg.wiki_id,r.wiki_id) AS wiki_id,w.name AS wiki_name,COALESCE(pg.path,r.path) AS path FROM memory_evidence e LEFT JOIN chat_threads t ON t.id=e.thread_id LEFT JOIN wiki_page_revisions r ON r.page_id=e.page_id AND r.revision=e.revision LEFT JOIN wiki_pages pg ON pg.id=e.page_id LEFT JOIN wikis w ON w.id=COALESCE(pg.wiki_id,r.wiki_id) WHERE e.memory_id=ANY($1::UUID[]) ORDER BY e.id",
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
        p1: &'a T1,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 1> {
        RecordQuery {
            client,
            params: [p1],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        memory_id: row.try_get(0)?,
                        thread_id: row.try_get(1)?,
                        agent_id: row.try_get(2)?,
                        message_id: row.try_get(3)?,
                        page_id: row.try_get(4)?,
                        revision: row.try_get(5)?,
                        wiki_id: row.try_get(6)?,
                        wiki_name: row.try_get(7)?,
                        path: row.try_get(8)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
