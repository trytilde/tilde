// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub slug: String,
    pub name: String,
    pub kind: String,
    pub catalog_group: Option<String>,
    pub repository_url: Option<String>,
    pub git_ref: Option<String>,
    pub git_path: String,
    pub commit_sha: Option<String>,
    pub sync_error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub skill_count: i32,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub slug: &'a str,
    pub name: &'a str,
    pub kind: &'a str,
    pub catalog_group: Option<&'a str>,
    pub repository_url: Option<&'a str>,
    pub git_ref: Option<&'a str>,
    pub git_path: &'a str,
    pub commit_sha: Option<&'a str>,
    pub sync_error: Option<&'a str>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub skill_count: i32,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            slug,
            name,
            kind,
            catalog_group,
            repository_url,
            git_ref,
            git_path,
            commit_sha,
            sync_error,
            created_at,
            skill_count,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            slug: slug.into(),
            name: name.into(),
            kind: kind.into(),
            catalog_group: catalog_group.map(|v| v.into()),
            repository_url: repository_url.map(|v| v.into()),
            git_ref: git_ref.map(|v| v.into()),
            git_path: git_path.into(),
            commit_sha: commit_sha.map(|v| v.into()),
            sync_error: sync_error.map(|v| v.into()),
            created_at,
            skill_count,
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
        "SELECT src.id,src.slug,src.name,src.kind,src.catalog_group,src.repository_url,src.git_ref,src.git_path,src.commit_sha,src.sync_error,src.created_at, (SELECT COUNT(*) FROM skills s WHERE s.source_id=src.id)::INTEGER AS skill_count FROM skill_sources src WHERE src.id=$1",
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
        id: &'a uuid::Uuid,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 1> {
        RecordQuery {
            client,
            params: [id],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        id: row.try_get(0)?,
                        slug: row.try_get(1)?,
                        name: row.try_get(2)?,
                        kind: row.try_get(3)?,
                        catalog_group: row.try_get(4)?,
                        repository_url: row.try_get(5)?,
                        git_ref: row.try_get(6)?,
                        git_path: row.try_get(7)?,
                        commit_sha: row.try_get(8)?,
                        sync_error: row.try_get(9)?,
                        created_at: row.try_get(10)?,
                        skill_count: row.try_get(11)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
