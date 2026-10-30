// This file was generated with `cornucopia`. Do not modify.

#[derive(Clone, Copy, Debug)]
pub struct RunParams {
    pub p2: uuid::Uuid,
    pub p1: uuid::Uuid,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "INSERT INTO memory_synthesis_threads(bank_id,thread_id,through_at,through_id) SELECT $1,p.thread_id,n.created_at,n.id FROM chat_participants p CROSS JOIN LATERAL (SELECT m.created_at,m.id FROM chat_messages m WHERE m.thread_id=p.thread_id ORDER BY m.created_at DESC,m.id DESC LIMIT 1) n WHERE p.agent_id=$2 AND NOT EXISTS (SELECT 1 FROM chat_participants o JOIN agent_memory_banks l ON l.agent_id=o.agent_id AND l.bank_id=$1 AND l.auto_retain WHERE o.thread_id=p.thread_id AND o.agent_id<>$2) ON CONFLICT(bank_id,thread_id) DO UPDATE SET through_at=EXCLUDED.through_at,through_id=EXCLUDED.through_id",
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
    pub async fn bind<'c, 'a, 's, C: GenericClient>(
        &'s self,
        client: &'c C,
        p2: &'a uuid::Uuid,
        p1: &'a uuid::Uuid,
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[p2, p1]).await
    }
}
impl<'a, C: GenericClient + Send + Sync>
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        RunParams,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for RunStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a RunParams,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(client, &params.p2, &params.p1))
    }
}
