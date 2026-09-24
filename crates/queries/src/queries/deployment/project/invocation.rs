// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::StringSql> {
    pub p1: uuid::Uuid,
    pub p2: uuid::Uuid,
    pub p3: uuid::Uuid,
    pub p4: uuid::Uuid,
    pub p5: T1,
    pub p6: chrono::DateTime<chrono::Utc>,
    pub p7: Option<chrono::DateTime<chrono::Utc>>,
    pub p8: Option<uuid::Uuid>,
    pub p9: Option<uuid::Uuid>,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "INSERT INTO chat_invocations(id,run_id,thread_id,agent_id,status,started_at,ended_at,lease_expires_at,deployment_id,history_through_message_id) VALUES($1,$2,$3,$4,$5,CASE WHEN $5<>'pending' THEN $6::timestamptz END,CASE WHEN $5 IN ('stopped','failed','canceled') THEN $6::timestamptz END,$7,$8,$9) ON CONFLICT(id) DO UPDATE SET status=EXCLUDED.status,ended_at=EXCLUDED.ended_at,lease_expires_at=EXCLUDED.lease_expires_at,history_through_message_id=EXCLUDED.history_through_message_id",
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
        p2: &'a uuid::Uuid,
        p3: &'a uuid::Uuid,
        p4: &'a uuid::Uuid,
        p5: &'a T1,
        p6: &'a chrono::DateTime<chrono::Utc>,
        p7: &'a Option<chrono::DateTime<chrono::Utc>>,
        p8: &'a Option<uuid::Uuid>,
        p9: &'a Option<uuid::Uuid>,
    ) -> Result<u64, tokio_postgres::Error> {
        client
            .execute(self.0, &[p1, p2, p3, p4, p5, p6, p7, p8, p9])
            .await
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
            client, &params.p1, &params.p2, &params.p3, &params.p4, &params.p5, &params.p6,
            &params.p7, &params.p8, &params.p9,
        ))
    }
}
