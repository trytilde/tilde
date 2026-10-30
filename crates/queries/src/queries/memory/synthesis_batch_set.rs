// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::StringSql, T2: crate::StringSql> {
    pub p1: uuid::Uuid,
    pub p2: uuid::Uuid,
    pub p3: uuid::Uuid,
    pub p4: Option<uuid::Uuid>,
    pub p5: Option<uuid::Uuid>,
    pub p6: Option<i32>,
    pub p7: T1,
    pub p8: bool,
    pub p9: i32,
    pub p10: i32,
    pub p11: i32,
    pub p12: i32,
    pub p13: T2,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct UuidUuidQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<uuid::Uuid, tokio_postgres::Error>,
    mapper: fn(uuid::Uuid) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> UuidUuidQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(self, mapper: fn(uuid::Uuid) -> R) -> UuidUuidQuery<'c, 'a, 's, C, R, N> {
        UuidUuidQuery {
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
        "INSERT INTO memory_synthesis_batches(id,bank_id,start_id,thread_id,page_id,revision,status,failures,written,superseded,forgotten,dropped,notes) VALUES($1,$2,$3,$4,$5,$6,$7,CASE WHEN $8::BOOLEAN THEN 1 ELSE 0 END,$9,$10,$11,$12,left($13,4000)) ON CONFLICT(id) DO UPDATE SET status=EXCLUDED.status, failures=memory_synthesis_batches.failures+EXCLUDED.failures,written=EXCLUDED.written, superseded=EXCLUDED.superseded,forgotten=EXCLUDED.forgotten,dropped=EXCLUDED.dropped,notes=EXCLUDED.notes,at=NOW() WHERE memory_synthesis_batches.status='failed' RETURNING id",
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
        p1: &'a uuid::Uuid,
        p2: &'a uuid::Uuid,
        p3: &'a uuid::Uuid,
        p4: &'a Option<uuid::Uuid>,
        p5: &'a Option<uuid::Uuid>,
        p6: &'a Option<i32>,
        p7: &'a T1,
        p8: &'a bool,
        p9: &'a i32,
        p10: &'a i32,
        p11: &'a i32,
        p12: &'a i32,
        p13: &'a T2,
    ) -> UuidUuidQuery<'c, 'a, 's, C, uuid::Uuid, 13> {
        UuidUuidQuery {
            client,
            params: [p1, p2, p3, p4, p5, p6, p7, p8, p9, p10, p11, p12, p13],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row| Ok(row.try_get(0)?),
            mapper: |it| it,
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::StringSql, T2: crate::StringSql>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        RunParams<T1, T2>,
        UuidUuidQuery<'c, 'a, 's, C, uuid::Uuid, 13>,
        C,
    > for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1, T2>,
    ) -> UuidUuidQuery<'c, 'a, 's, C, uuid::Uuid, 13> {
        self.bind(
            client,
            &params.p1,
            &params.p2,
            &params.p3,
            &params.p4,
            &params.p5,
            &params.p6,
            &params.p7,
            &params.p8,
            &params.p9,
            &params.p10,
            &params.p11,
            &params.p12,
            &params.p13,
        )
    }
}
