// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<
    T1: crate::StringSql,
    T2: crate::StringSql,
    T3: crate::StringSql,
    T4: crate::StringSql,
> {
    pub name: T1,
    pub prompt: T2,
    pub enabled: bool,
    pub schedule: Option<T3>,
    pub connection: Option<uuid::Uuid>,
    pub signal_type: Option<T4>,
    pub next_run_at: Option<chrono::DateTime<chrono::Utc>>,
    pub id: uuid::Uuid,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "UPDATE routines SET name=$1,prompt=$2,enabled=$3,schedule=$4,connection_id=$5, signal_type=$6,next_run_at=$7,updated_at=NOW() WHERE id=$8",
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
    >(
        &'s self,
        client: &'c C,
        name: &'a T1,
        prompt: &'a T2,
        enabled: &'a bool,
        schedule: &'a Option<T3>,
        connection: &'a Option<uuid::Uuid>,
        signal_type: &'a Option<T4>,
        next_run_at: &'a Option<chrono::DateTime<chrono::Utc>>,
        id: &'a uuid::Uuid,
    ) -> Result<u64, tokio_postgres::Error> {
        client
            .execute(
                self.0,
                &[
                    name,
                    prompt,
                    enabled,
                    schedule,
                    connection,
                    signal_type,
                    next_run_at,
                    id,
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
            client,
            &params.name,
            &params.prompt,
            &params.enabled,
            &params.schedule,
            &params.connection,
            &params.signal_type,
            &params.next_run_at,
            &params.id,
        ))
    }
}
