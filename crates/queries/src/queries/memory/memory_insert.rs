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
    pub p2: uuid::Uuid,
    pub p3: T1,
    pub p4: T2,
    pub p5: T3,
    pub p6: T4,
    pub p7: Option<chrono::DateTime<chrono::Utc>>,
    pub p15: bool,
    pub p8: Option<chrono::DateTime<chrono::Utc>>,
    pub p9: f32,
    pub p10: T5,
    pub p11: Option<uuid::Uuid>,
    pub p12: Option<uuid::Uuid>,
    pub p13: Option<uuid::Uuid>,
    pub p14: Option<uuid::Uuid>,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "INSERT INTO memories(id,bank_id,kind,path,title,content,valid_during,importance,origin,identity_id,written_by_agent_id,supersedes_id,thread_id,message_id) VALUES($1,$2,$3,$4,$5,$6, CASE WHEN $7::TIMESTAMPTZ IS NULL THEN NULL WHEN $8::BOOLEAN THEN tstzrange($7,NULL,'[)') WHEN $9::TIMESTAMPTZ IS NULL OR $9=$7 THEN tstzrange($7,$7,'[]') ELSE tstzrange($7,$9,'[)') END, $10,$11,$12,$13,$14, (SELECT id FROM chat_threads WHERE id=$15::UUID), (SELECT m.id FROM chat_messages m JOIN chat_participants p ON p.id=m.participant_id WHERE m.thread_id=$15 AND p.user_id IS NOT NULL ORDER BY m.created_at DESC LIMIT 1))",
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
        p2: &'a uuid::Uuid,
        p3: &'a T1,
        p4: &'a T2,
        p5: &'a T3,
        p6: &'a T4,
        p7: &'a Option<chrono::DateTime<chrono::Utc>>,
        p15: &'a bool,
        p8: &'a Option<chrono::DateTime<chrono::Utc>>,
        p9: &'a f32,
        p10: &'a T5,
        p11: &'a Option<uuid::Uuid>,
        p12: &'a Option<uuid::Uuid>,
        p13: &'a Option<uuid::Uuid>,
        p14: &'a Option<uuid::Uuid>,
    ) -> Result<u64, tokio_postgres::Error> {
        client
            .execute(
                self.0,
                &[
                    p1, p2, p3, p4, p5, p6, p7, p15, p8, p9, p10, p11, p12, p13, p14,
                ],
            )
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
            &params.p15,
            &params.p8,
            &params.p9,
            &params.p10,
            &params.p11,
            &params.p12,
            &params.p13,
            &params.p14,
        ))
    }
}
