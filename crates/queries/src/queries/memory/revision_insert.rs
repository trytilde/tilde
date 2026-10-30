// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::StringSql, T2: crate::StringSql> {
    pub p2: T1,
    pub p3: T2,
    pub p4: Option<uuid::Uuid>,
    pub p5: Option<uuid::Uuid>,
    pub p1: uuid::Uuid,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "INSERT INTO memory_revisions(memory_id,operation,kind,path,title,content,valid_during,importance,origin,identity_id,actor,agent_id,batch_id) SELECT id,$1,kind,path,title,content,valid_during,importance,origin,identity_id,$2,$3,$4 FROM memories WHERE id=$5",
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
    pub async fn bind<'c, 'a, 's, C: GenericClient, T1: crate::StringSql, T2: crate::StringSql>(
        &'s self,
        client: &'c C,
        p2: &'a T1,
        p3: &'a T2,
        p4: &'a Option<uuid::Uuid>,
        p5: &'a Option<uuid::Uuid>,
        p1: &'a uuid::Uuid,
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[p2, p3, p4, p5, p1]).await
    }
}
impl<'a, C: GenericClient + Send + Sync, T1: crate::StringSql, T2: crate::StringSql>
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
        Box::pin(self.bind(
            client, &params.p2, &params.p3, &params.p4, &params.p5, &params.p1,
        ))
    }
}
