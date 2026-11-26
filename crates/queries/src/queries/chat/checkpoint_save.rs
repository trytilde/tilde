// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::StringSql, T2: crate::ArraySql<Item = uuid::Uuid>> {
    pub p1: uuid::Uuid,
    pub p2: uuid::Uuid,
    pub p3: T1,
    pub p4: T2,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "INSERT INTO chat_run_checkpoints(run_id, invocation_id, state_json, pending_call_ids) VALUES ($1, $2, $3, $4) ON CONFLICT (run_id) DO UPDATE SET invocation_id = EXCLUDED.invocation_id, state_json = EXCLUDED.state_json, pending_call_ids = EXCLUDED.pending_call_ids, created_at = NOW()",
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
        T2: crate::ArraySql<Item = uuid::Uuid>,
    >(
        &'s self,
        client: &'c C,
        p1: &'a uuid::Uuid,
        p2: &'a uuid::Uuid,
        p3: &'a T1,
        p4: &'a T2,
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[p1, p2, p3, p4]).await
    }
}
impl<
    'a,
    C: GenericClient + Send + Sync,
    T1: crate::StringSql,
    T2: crate::ArraySql<Item = uuid::Uuid>,
>
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        RunParams<T1, T2>,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for RunStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a RunParams<T1, T2>,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(client, &params.p1, &params.p2, &params.p3, &params.p4))
    }
}
