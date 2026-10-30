// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub agent_id: uuid::Uuid,
    pub wiki_id: uuid::Uuid,
    pub wiki_name: String,
    pub description: String,
    pub access: String,
    pub own: bool,
}
pub struct RecordBorrowed<'a> {
    pub agent_id: uuid::Uuid,
    pub wiki_id: uuid::Uuid,
    pub wiki_name: &'a str,
    pub description: &'a str,
    pub access: &'a str,
    pub own: bool,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            agent_id,
            wiki_id,
            wiki_name,
            description,
            access,
            own,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            agent_id,
            wiki_id,
            wiki_name: wiki_name.into(),
            description: description.into(),
            access: access.into(),
            own,
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
        "SELECT l.agent_id,l.wiki_id,w.name AS wiki_name,w.description,l.access,COALESCE(w.agent_id=l.agent_id,FALSE) AS own FROM agent_wikis l JOIN wikis w ON w.id=l.wiki_id WHERE l.agent_id=$1 ORDER BY w.name",
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
                        agent_id: row.try_get(0)?,
                        wiki_id: row.try_get(1)?,
                        wiki_name: row.try_get(2)?,
                        description: row.try_get(3)?,
                        access: row.try_get(4)?,
                        own: row.try_get(5)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
