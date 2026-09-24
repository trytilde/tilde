// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::StringSql, T2: crate::StringSql> {
    pub p1: T1,
    pub p2: T2,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub provider_id: String,
    pub model: String,
    pub input_per_m_micros: Option<i64>,
    pub output_per_m_micros: Option<i64>,
    pub cached_input_per_m_micros: Option<i64>,
    pub cache_write_per_m_micros: Option<i64>,
    pub image_micros: Option<i64>,
    pub character_per_m_micros: Option<i64>,
    pub second_micros: Option<i64>,
    pub source: String,
}
pub struct RecordBorrowed<'a> {
    pub provider_id: &'a str,
    pub model: &'a str,
    pub input_per_m_micros: Option<i64>,
    pub output_per_m_micros: Option<i64>,
    pub cached_input_per_m_micros: Option<i64>,
    pub cache_write_per_m_micros: Option<i64>,
    pub image_micros: Option<i64>,
    pub character_per_m_micros: Option<i64>,
    pub second_micros: Option<i64>,
    pub source: &'a str,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            provider_id,
            model,
            input_per_m_micros,
            output_per_m_micros,
            cached_input_per_m_micros,
            cache_write_per_m_micros,
            image_micros,
            character_per_m_micros,
            second_micros,
            source,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            model: model.into(),
            input_per_m_micros,
            output_per_m_micros,
            cached_input_per_m_micros,
            cache_write_per_m_micros,
            image_micros,
            character_per_m_micros,
            second_micros,
            source: source.into(),
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
        "SELECT provider_id,model,input_per_m_micros,output_per_m_micros,cached_input_per_m_micros,cache_write_per_m_micros,image_micros,character_per_m_micros,second_micros,source FROM inference_prices WHERE provider_id=$1 AND model=$2",
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
    pub fn bind<'c, 'a, 's, C: GenericClient, T1: crate::StringSql, T2: crate::StringSql>(
        &'s self,
        client: &'c C,
        p1: &'a T1,
        p2: &'a T2,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        RecordQuery {
            client,
            params: [p1, p2],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        provider_id: row.try_get(0)?,
                        model: row.try_get(1)?,
                        input_per_m_micros: row.try_get(2)?,
                        output_per_m_micros: row.try_get(3)?,
                        cached_input_per_m_micros: row.try_get(4)?,
                        cache_write_per_m_micros: row.try_get(5)?,
                        image_micros: row.try_get(6)?,
                        character_per_m_micros: row.try_get(7)?,
                        second_micros: row.try_get(8)?,
                        source: row.try_get(9)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::StringSql, T2: crate::StringSql>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        RunParams<T1, T2>,
        RecordQuery<'c, 'a, 's, C, Record, 2>,
        C,
    > for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1, T2>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        self.bind(client, &params.p1, &params.p2)
    }
}
