// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<
    T1: crate::StringSql,
    T2: crate::StringSql,
    T3: crate::StringSql,
    T4: crate::StringSql,
    T5: crate::StringSql,
    T6: crate::BytesSql,
    T7: crate::StringSql,
    T8: crate::ArraySql<Item = T7>,
    T9: crate::StringSql,
    T10: crate::StringSql,
    T11: crate::StringSql,
> {
    pub p1: T1,
    pub p2: T2,
    pub p3: T3,
    pub p4: Option<T4>,
    pub p5: Option<T5>,
    pub p6: Option<T6>,
    pub p7: Option<uuid::Uuid>,
    pub p8: T8,
    pub p9: Option<T9>,
    pub p10: Option<T10>,
    pub p11: Option<T11>,
    pub p12: Option<uuid::Uuid>,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "INSERT INTO connection_providers(provider_id,name,kind,remote_endpoint,remote_ui_url,remote_authorization,remote_authorization_id,categories,icon_url,instructions,account_name_label,tool_host_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) ON CONFLICT(provider_id) DO UPDATE SET name=excluded.name,kind=excluded.kind,remote_endpoint=excluded.remote_endpoint,remote_ui_url=excluded.remote_ui_url,remote_authorization=excluded.remote_authorization,remote_authorization_id=excluded.remote_authorization_id,categories=excluded.categories,icon_url=excluded.icon_url,instructions=excluded.instructions,account_name_label=excluded.account_name_label,tool_host_id=excluded.tool_host_id",
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
        T6: crate::BytesSql,
        T7: crate::StringSql,
        T8: crate::ArraySql<Item = T7>,
        T9: crate::StringSql,
        T10: crate::StringSql,
        T11: crate::StringSql,
    >(
        &'s self,
        client: &'c C,
        p1: &'a T1,
        p2: &'a T2,
        p3: &'a T3,
        p4: &'a Option<T4>,
        p5: &'a Option<T5>,
        p6: &'a Option<T6>,
        p7: &'a Option<uuid::Uuid>,
        p8: &'a T8,
        p9: &'a Option<T9>,
        p10: &'a Option<T10>,
        p11: &'a Option<T11>,
        p12: &'a Option<uuid::Uuid>,
    ) -> Result<u64, tokio_postgres::Error> {
        client
            .execute(self.0, &[p1, p2, p3, p4, p5, p6, p7, p8, p9, p10, p11, p12])
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
    T6: crate::BytesSql,
    T7: crate::StringSql,
    T8: crate::ArraySql<Item = T7>,
    T9: crate::StringSql,
    T10: crate::StringSql,
    T11: crate::StringSql,
>
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        RunParams<T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11>,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for RunStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a RunParams<T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11>,
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
            &params.p8,
            &params.p9,
            &params.p10,
            &params.p11,
            &params.p12,
        ))
    }
}
