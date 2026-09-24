// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<
    T1: crate::BytesSql,
    T2: crate::BytesSql,
    T3: crate::BytesSql,
    T4: crate::BytesSql,
> {
    pub p1: uuid::Uuid,
    pub p2: uuid::Uuid,
    pub p3: uuid::Uuid,
    pub p4: T1,
    pub p5: T2,
    pub p6: T3,
    pub p7: T4,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "INSERT INTO connection_setups(id,connection_id,step,action_id,connection_setup_token,connection_setup_token_hash,callback_token,callback_hash,expires_at) VALUES($1,$2,'fields',$3,$4,$5,$6,$7,now()+interval '10 minutes')",
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
        T1: crate::BytesSql,
        T2: crate::BytesSql,
        T3: crate::BytesSql,
        T4: crate::BytesSql,
    >(
        &'s self,
        client: &'c C,
        p1: &'a uuid::Uuid,
        p2: &'a uuid::Uuid,
        p3: &'a uuid::Uuid,
        p4: &'a T1,
        p5: &'a T2,
        p6: &'a T3,
        p7: &'a T4,
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[p1, p2, p3, p4, p5, p6, p7]).await
    }
}
impl<
    'a,
    C: GenericClient + Send + Sync,
    T1: crate::BytesSql,
    T2: crate::BytesSql,
    T3: crate::BytesSql,
    T4: crate::BytesSql,
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
            client, &params.p1, &params.p2, &params.p3, &params.p4, &params.p5, &params.p6,
            &params.p7,
        ))
    }
}
