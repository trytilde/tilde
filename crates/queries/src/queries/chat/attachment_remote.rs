// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<
    T1: crate::StringSql,
    T2: crate::StringSql,
    T3: crate::StringSql,
    T4: crate::BytesSql,
> {
    pub p1: uuid::Uuid,
    pub p2: uuid::Uuid,
    pub p3: T1,
    pub p4: T2,
    pub p5: i64,
    pub p6: uuid::Uuid,
    pub p7: Option<T3>,
    pub p8: Option<T4>,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "INSERT INTO chat_attachments(id,thread_id,filename,media_type,size_bytes,sha256,connection_id,provider_attachment_id,source_url) VALUES($1,$2,$3,$4,$5,'',$6,$7,$8) ON CONFLICT DO NOTHING",
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
        T4: crate::BytesSql,
    >(
        &'s self,
        client: &'c C,
        p1: &'a uuid::Uuid,
        p2: &'a uuid::Uuid,
        p3: &'a T1,
        p4: &'a T2,
        p5: &'a i64,
        p6: &'a uuid::Uuid,
        p7: &'a Option<T3>,
        p8: &'a Option<T4>,
    ) -> Result<u64, tokio_postgres::Error> {
        client
            .execute(self.0, &[p1, p2, p3, p4, p5, p6, p7, p8])
            .await
    }
}
impl<
    'a,
    C: GenericClient + Send + Sync,
    T1: crate::StringSql,
    T2: crate::StringSql,
    T3: crate::StringSql,
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
            &params.p7, &params.p8,
        ))
    }
}
