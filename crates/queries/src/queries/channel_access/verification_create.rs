// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::BytesSql> {
    pub p1: uuid::Uuid,
    pub p2: uuid::Uuid,
    pub p3: uuid::Uuid,
    pub p4: uuid::Uuid,
    pub p5: T1,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "INSERT INTO chat_identity_verifications(id,connection_id,agent_id,identity_id,token_hash,status,expires_at) VALUES($1,$2,$3,$4,$5,'pending',NOW()+INTERVAL '10 minutes')",
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
    pub async fn bind<'c, 'a, 's, C: GenericClient, T1: crate::BytesSql>(
        &'s self,
        client: &'c C,
        p1: &'a uuid::Uuid,
        p2: &'a uuid::Uuid,
        p3: &'a uuid::Uuid,
        p4: &'a uuid::Uuid,
        p5: &'a T1,
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[p1, p2, p3, p4, p5]).await
    }
}
impl<'a, C: GenericClient + Send + Sync, T1: crate::BytesSql>
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        RunParams<T1>,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for RunStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a RunParams<T1>,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(
            client, &params.p1, &params.p2, &params.p3, &params.p4, &params.p5,
        ))
    }
}
