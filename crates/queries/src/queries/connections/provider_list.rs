// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<
    T1: crate::StringSql,
    T2: crate::StringSql,
    T3: crate::StringSql,
    T4: crate::StringSql,
    T5: crate::StringSql,
> {
    pub p1: T1,
    pub p2: Option<T2>,
    pub p5: Option<T3>,
    pub p4: Option<T4>,
    pub p6: Option<T5>,
    pub p3: i64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub provider_id: String,
    pub name: String,
    pub kind: String,
    pub categories: Vec<String>,
}
pub struct RecordBorrowed<'a> {
    pub provider_id: &'a str,
    pub name: &'a str,
    pub kind: &'a str,
    pub categories: crate::ArrayIterator<'a, &'a str>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            provider_id,
            name,
            kind,
            categories,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            name: name.into(),
            kind: kind.into(),
            categories: categories.map(|v| v.into()).collect(),
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
        "SELECT provider_id,name,kind,categories FROM connection_providers p WHERE provider_id>$1 AND ($2::TEXT IS NULL OR name ILIKE '%'||$2||'%' OR provider_id ILIKE '%'||$2||'%' OR instructions ILIKE '%'||$2||'%') AND ($3::TEXT IS NULL OR $3=ANY(categories)) AND ($4::TEXT IS NULL OR EXISTS(SELECT 1 FROM connection_types t WHERE t.provider_id=p.provider_id AND CASE $4 WHEN 'channel' THEN t.channel_capable WHEN 'inference' THEN t.inference_capable WHEN 'tool' THEN t.tool_capable WHEN 'signal' THEN t.signal_capable END)) AND ($5::TEXT IS NULL OR provider_source(p.provider_id)=$5) ORDER BY provider_id LIMIT $6",
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
        T1: crate::StringSql,
        T2: crate::StringSql,
        T3: crate::StringSql,
        T4: crate::StringSql,
        T5: crate::StringSql,
    >(
        &'s self,
        client: &'c C,
        p1: &'a T1,
        p2: &'a Option<T2>,
        p5: &'a Option<T3>,
        p4: &'a Option<T4>,
        p6: &'a Option<T5>,
        p3: &'a i64,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 6> {
        RecordQuery {
            client,
            params: [p1, p2, p5, p4, p6, p3],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        provider_id: row.try_get(0)?,
                        name: row.try_get(1)?,
                        kind: row.try_get(2)?,
                        categories: row.try_get(3)?,
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
    T1: crate::StringSql,
    T2: crate::StringSql,
    T3: crate::StringSql,
    T4: crate::StringSql,
    T5: crate::StringSql,
>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        RunParams<T1, T2, T3, T4, T5>,
        RecordQuery<'c, 'a, 's, C, Record, 6>,
        C,
    > for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1, T2, T3, T4, T5>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 6> {
        self.bind(
            client, &params.p1, &params.p2, &params.p5, &params.p4, &params.p6, &params.p3,
        )
    }
}
