// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::StringSql> {
    pub p2: f64,
    pub p1: Option<uuid::Uuid>,
    pub p3: Option<T1>,
    pub p4: Option<bool>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub name: String,
    pub execution_type: String,
    pub function_arn: Option<String>,
    pub provider_id: Option<String>,
    pub available: bool,
    pub auth_methods: Vec<String>,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub name: &'a str,
    pub execution_type: &'a str,
    pub function_arn: Option<&'a str>,
    pub provider_id: Option<&'a str>,
    pub available: bool,
    pub auth_methods: crate::ArrayIterator<'a, &'a str>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            name,
            execution_type,
            function_arn,
            provider_id,
            available,
            auth_methods,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            execution_type: execution_type.into(),
            function_arn: function_arn.map(|v| v.into()),
            provider_id: provider_id.map(|v| v.into()),
            available,
            auth_methods: auth_methods.map(|v| v.into()).collect(),
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
        "SELECT h.id,h.name,h.execution_type,h.function_arn,p.provider_id, (h.execution_type='lambda' OR COALESCE(h.connected_at > NOW() - make_interval(secs => $1), false)) AS available, COALESCE((SELECT array_agg(t.name ORDER BY t.name) FROM connection_types t WHERE t.provider_id=p.provider_id), '{}')::TEXT[] AS auth_methods FROM tool_hosts h LEFT JOIN connection_providers p ON p.tool_host_id=h.id WHERE ($2::UUID IS NULL OR h.id=$2) AND ($3::TEXT IS NULL OR h.name ILIKE '%'||$3||'%') AND ($4::BOOLEAN IS NULL OR (p.provider_id IS NOT NULL)=$4) ORDER BY h.name",
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
        p2: &'a f64,
        p1: &'a Option<uuid::Uuid>,
        p3: &'a Option<T1>,
        p4: &'a Option<bool>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 4> {
        RecordQuery {
            client,
            params: [p2, p1, p3, p4],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        id: row.try_get(0)?,
                        name: row.try_get(1)?,
                        execution_type: row.try_get(2)?,
                        function_arn: row.try_get(3)?,
                        provider_id: row.try_get(4)?,
                        available: row.try_get(5)?,
                        auth_methods: row.try_get(6)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        RunParams<T1>,
        RecordQuery<'c, 'a, 's, C, Record, 4>,
        C,
    > for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 4> {
        self.bind(client, &params.p2, &params.p1, &params.p3, &params.p4)
    }
}
