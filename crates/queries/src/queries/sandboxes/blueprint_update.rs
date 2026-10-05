// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::StringSql, T2: crate::StringSql, T3: crate::StringSql> {
    pub p2: Option<T1>,
    pub p3: Option<uuid::Uuid>,
    pub p4: Option<T2>,
    pub p5: Option<T3>,
    pub p6: Option<i32>,
    pub p7: Option<i32>,
    pub p8: Option<i32>,
    pub p1: uuid::Uuid,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "UPDATE sandbox_blueprints SET name=COALESCE($1,name),connection_id=COALESCE($2,connection_id), template=COALESCE($3,template),reuse=COALESCE($4,reuse),sleep_after_secs=COALESCE($5,sleep_after_secs), terminate_after_secs=COALESCE($6,terminate_after_secs),connect_timeout_secs=COALESCE($7,connect_timeout_secs) WHERE id=$8 AND NOT EXISTS (SELECT 1 FROM sandbox_blueprints o WHERE o.name=$1 AND o.id<>$8)",
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
    >(
        &'s self,
        client: &'c C,
        p2: &'a Option<T1>,
        p3: &'a Option<uuid::Uuid>,
        p4: &'a Option<T2>,
        p5: &'a Option<T3>,
        p6: &'a Option<i32>,
        p7: &'a Option<i32>,
        p8: &'a Option<i32>,
        p1: &'a uuid::Uuid,
    ) -> Result<u64, tokio_postgres::Error> {
        client
            .execute(self.0, &[p2, p3, p4, p5, p6, p7, p8, p1])
            .await
    }
}
impl<
    'a,
    C: GenericClient + Send + Sync,
    T1: crate::StringSql,
    T2: crate::StringSql,
    T3: crate::StringSql,
>
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        RunParams<T1, T2, T3>,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for RunStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a RunParams<T1, T2, T3>,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(
            client, &params.p2, &params.p3, &params.p4, &params.p5, &params.p6, &params.p7,
            &params.p8, &params.p1,
        ))
    }
}
