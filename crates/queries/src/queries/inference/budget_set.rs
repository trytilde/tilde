// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::StringSql, T2: crate::StringSql, T3: crate::StringSql> {
    pub p1: uuid::Uuid,
    pub p2: T1,
    pub p3: uuid::Uuid,
    pub p4: Option<uuid::Uuid>,
    pub p5: T2,
    pub p6: i64,
    pub p7: T3,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub scope: String,
    pub scope_id: uuid::Uuid,
    pub connection_id: Option<uuid::Uuid>,
    pub period: String,
    pub limit_micros: i64,
    pub action: String,
    pub spent_micros: i64,
    pub exhausted_until: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub scope: &'a str,
    pub scope_id: uuid::Uuid,
    pub connection_id: Option<uuid::Uuid>,
    pub period: &'a str,
    pub limit_micros: i64,
    pub action: &'a str,
    pub spent_micros: i64,
    pub exhausted_until: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            scope,
            scope_id,
            connection_id,
            period,
            limit_micros,
            action,
            spent_micros,
            exhausted_until,
            created_at,
            updated_at,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            scope: scope.into(),
            scope_id,
            connection_id,
            period: period.into(),
            limit_micros,
            action: action.into(),
            spent_micros,
            exhausted_until,
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
        "INSERT INTO inference_budgets(id,scope,scope_id,connection_id,period,limit_micros,action) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT (scope,scope_id,period,COALESCE(connection_id,'00000000-0000-0000-0000-000000000000')) DO UPDATE SET limit_micros=excluded.limit_micros,action=excluded.action,exhausted_until=NULL,updated_at=NOW() RETURNING id,scope,scope_id,connection_id,period,limit_micros,action,spent_micros,exhausted_until,created_at,updated_at",
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
    >(
        &'s self,
        client: &'c C,
        p1: &'a uuid::Uuid,
        p2: &'a T1,
        p3: &'a uuid::Uuid,
        p4: &'a Option<uuid::Uuid>,
        p5: &'a T2,
        p6: &'a i64,
        p7: &'a T3,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 7> {
        RecordQuery {
            client,
            params: [p1, p2, p3, p4, p5, p6, p7],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        id: row.try_get(0)?,
                        scope: row.try_get(1)?,
                        scope_id: row.try_get(2)?,
                        connection_id: row.try_get(3)?,
                        period: row.try_get(4)?,
                        limit_micros: row.try_get(5)?,
                        action: row.try_get(6)?,
                        spent_micros: row.try_get(7)?,
                        exhausted_until: row.try_get(8)?,
                        created_at: row.try_get(9)?,
                        updated_at: row.try_get(10)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::StringSql, T2: crate::StringSql, T3: crate::StringSql>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        RunParams<T1, T2, T3>,
        RecordQuery<'c, 'a, 's, C, Record, 7>,
        C,
    > for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1, T2, T3>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 7> {
        self.bind(
            client, &params.p1, &params.p2, &params.p3, &params.p4, &params.p5, &params.p6,
            &params.p7,
        )
    }
}
