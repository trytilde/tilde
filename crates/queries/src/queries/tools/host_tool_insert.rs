// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<
    T1: crate::StringSql,
    T2: crate::StringSql,
    T3: crate::StringSql,
    T4: crate::StringSql,
    T5: crate::StringSql,
> {
    pub p1: uuid::Uuid,
    pub p2: T1,
    pub p3: T2,
    pub p4: T3,
    pub p5: T4,
    pub p6: T5,
    pub p7: bool,
    pub p8: bool,
    pub p9: bool,
    pub p10: bool,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "INSERT INTO tool_host_tools(tool_host_id,name,description,summary,input_schema_json,output_schema_json,read_only,destructive,idempotent,open_world) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
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
        T5: crate::StringSql,
    >(
        &'s self,
        client: &'c C,
        p1: &'a uuid::Uuid,
        p2: &'a T1,
        p3: &'a T2,
        p4: &'a T3,
        p5: &'a T4,
        p6: &'a T5,
        p7: &'a bool,
        p8: &'a bool,
        p9: &'a bool,
        p10: &'a bool,
    ) -> Result<u64, tokio_postgres::Error> {
        client
            .execute(self.0, &[p1, p2, p3, p4, p5, p6, p7, p8, p9, p10])
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
    T5: crate::StringSql,
>
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        RunParams<T1, T2, T3, T4, T5>,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for RunStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a RunParams<T1, T2, T3, T4, T5>,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(
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
        ))
    }
}
