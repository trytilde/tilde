// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::ArraySql<Item = uuid::Uuid>, T2: crate::ArraySql<Item = uuid::Uuid>>
{
    pub request_ids: T1,
    pub version_ids: T2,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "INSERT INTO inference_request_prompts(request_id,version_id) SELECT * FROM UNNEST($1::UUID[],$2::UUID[]) ON CONFLICT DO NOTHING",
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
        T1: crate::ArraySql<Item = uuid::Uuid>,
        T2: crate::ArraySql<Item = uuid::Uuid>,
    >(
        &'s self,
        client: &'c C,
        request_ids: &'a T1,
        version_ids: &'a T2,
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[request_ids, version_ids]).await
    }
}
impl<
    'a,
    C: GenericClient + Send + Sync,
    T1: crate::ArraySql<Item = uuid::Uuid>,
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
        Box::pin(self.bind(client, &params.request_ids, &params.version_ids))
    }
}
