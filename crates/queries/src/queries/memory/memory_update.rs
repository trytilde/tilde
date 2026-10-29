// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<
    T1: crate::StringSql,
    T2: crate::StringSql,
    T3: crate::StringSql,
    T4: crate::StringSql,
> {
    pub p2: Option<T1>,
    pub p3: Option<T2>,
    pub p4: Option<T3>,
    pub p5: Option<T4>,
    pub p8: bool,
    pub p6: Option<chrono::DateTime<chrono::Utc>>,
    pub p10: bool,
    pub p7: Option<chrono::DateTime<chrono::Utc>>,
    pub p9: Option<f32>,
    pub p1: uuid::Uuid,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "UPDATE memories SET kind=COALESCE($1,kind),path=COALESCE($2,path),title=COALESCE($3,title),content=COALESCE($4,content), valid_during=CASE WHEN $5 THEN NULL WHEN $6::TIMESTAMPTZ IS NULL THEN valid_during WHEN $7::BOOLEAN THEN tstzrange($6,NULL,'[)') WHEN $8::TIMESTAMPTZ IS NULL OR $8=$6 THEN tstzrange($6,$6,'[]') ELSE tstzrange($6,$8,'[)') END, importance=COALESCE($9,importance),origin='owner',updated_at=NOW() WHERE id=$10 AND deleted_at IS NULL",
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
    pub async fn bind<
        'c,
        'a,
        's,
        C: GenericClient,
        T1: crate::StringSql,
        T2: crate::StringSql,
        T3: crate::StringSql,
        T4: crate::StringSql,
    >(
        &'s self,
        client: &'c C,
        p2: &'a Option<T1>,
        p3: &'a Option<T2>,
        p4: &'a Option<T3>,
        p5: &'a Option<T4>,
        p8: &'a bool,
        p6: &'a Option<chrono::DateTime<chrono::Utc>>,
        p10: &'a bool,
        p7: &'a Option<chrono::DateTime<chrono::Utc>>,
        p9: &'a Option<f32>,
        p1: &'a uuid::Uuid,
    ) -> Result<u64, tokio_postgres::Error> {
        client
            .execute(self.0, &[p2, p3, p4, p5, p8, p6, p10, p7, p9, p1])
            .await
    }
}
impl<
    'a,
    C: GenericClient + Send + Sync,
    T1: crate::StringSql,
    T2: crate::StringSql,
    T3: crate::StringSql,
    T4: crate::StringSql,
>
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        RunParams<T1, T2, T3, T4>,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for RunStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a RunParams<T1, T2, T3, T4>,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(
            client,
            &params.p2,
            &params.p3,
            &params.p4,
            &params.p5,
            &params.p8,
            &params.p6,
            &params.p10,
            &params.p7,
            &params.p9,
            &params.p1,
        ))
    }
}
