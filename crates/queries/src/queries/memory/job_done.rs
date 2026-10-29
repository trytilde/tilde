// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::StringSql> {
    pub p1: uuid::Uuid,
    pub p2: chrono::DateTime<chrono::Utc>,
    pub p3: T1,
    pub p4: i32,
    pub p5: i32,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "WITH done AS (DELETE FROM memory_embedding_jobs WHERE memory_id=$1 AND requested_at=$2 RETURNING memory_id) UPDATE memories SET embedding=$3::TEXT::halfvec,embedding_dims=$4,embedding_version=$5 WHERE id IN (SELECT memory_id FROM done)",
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
    pub async fn bind<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>(
        &'s self,
        client: &'c C,
        p1: &'a uuid::Uuid,
        p2: &'a chrono::DateTime<chrono::Utc>,
        p3: &'a T1,
        p4: &'a i32,
        p5: &'a i32,
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[p1, p2, p3, p4, p5]).await
    }
}
impl<'a, C: GenericClient + Send + Sync, T1: crate::StringSql>
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
