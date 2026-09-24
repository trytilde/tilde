// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub provider_id: String,
    pub name: String,
    pub account_name_label: Option<String>,
    pub icon_url: Option<String>,
    pub instructions: Option<String>,
    pub kind: String,
    pub categories: Vec<String>,
    pub remote_endpoint: Option<String>,
    pub remote_ui_url: Option<String>,
}
pub struct RecordBorrowed<'a> {
    pub provider_id: &'a str,
    pub name: &'a str,
    pub account_name_label: Option<&'a str>,
    pub icon_url: Option<&'a str>,
    pub instructions: Option<&'a str>,
    pub kind: &'a str,
    pub categories: crate::ArrayIterator<'a, &'a str>,
    pub remote_endpoint: Option<&'a str>,
    pub remote_ui_url: Option<&'a str>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            provider_id,
            name,
            account_name_label,
            icon_url,
            instructions,
            kind,
            categories,
            remote_endpoint,
            remote_ui_url,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            name: name.into(),
            account_name_label: account_name_label.map(|v| v.into()),
            icon_url: icon_url.map(|v| v.into()),
            instructions: instructions.map(|v| v.into()),
            kind: kind.into(),
            categories: categories.map(|v| v.into()).collect(),
            remote_endpoint: remote_endpoint.map(|v| v.into()),
            remote_ui_url: remote_ui_url.map(|v| v.into()),
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
        "SELECT provider_id,name,account_name_label,icon_url,instructions,kind,categories,remote_endpoint,remote_ui_url FROM connection_providers WHERE provider_id=$1",
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
    pub fn bind<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>(
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
                        provider_id: row.try_get(0)?,
                        name: row.try_get(1)?,
                        account_name_label: row.try_get(2)?,
                        icon_url: row.try_get(3)?,
                        instructions: row.try_get(4)?,
                        kind: row.try_get(5)?,
                        categories: row.try_get(6)?,
                        remote_endpoint: row.try_get(7)?,
                        remote_ui_url: row.try_get(8)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
