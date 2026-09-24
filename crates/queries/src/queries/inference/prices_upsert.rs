// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<
    T1: crate::StringSql,
    T2: crate::ArraySql<Item = T1>,
    T3: crate::StringSql,
    T4: crate::ArraySql<Item = T3>,
    T5: crate::ArraySql<Item = i64>,
    T6: crate::ArraySql<Item = i64>,
    T7: crate::ArraySql<Item = i64>,
    T8: crate::ArraySql<Item = i64>,
    T9: crate::ArraySql<Item = i64>,
    T10: crate::ArraySql<Item = i64>,
    T11: crate::ArraySql<Item = i64>,
> {
    pub p1: T2,
    pub p2: T4,
    pub p3: T5,
    pub p4: T6,
    pub p5: T7,
    pub p6: T8,
    pub p7: T9,
    pub p8: T10,
    pub p9: T11,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "INSERT INTO inference_prices(provider_id,model,input_per_m_micros,output_per_m_micros,cached_input_per_m_micros,cache_write_per_m_micros,image_micros,character_per_m_micros,second_micros,source,updated_at) SELECT u.provider_id,u.model,NULLIF(u.input_per_m,-1),NULLIF(u.output_per_m,-1),NULLIF(u.cached_per_m,-1),NULLIF(u.write_per_m,-1),NULLIF(u.image,-1),NULLIF(u.character_per_m,-1),NULLIF(u.second,-1),'litellm',NOW() FROM UNNEST($1::TEXT[],$2::TEXT[],$3::BIGINT[],$4::BIGINT[],$5::BIGINT[],$6::BIGINT[],$7::BIGINT[],$8::BIGINT[],$9::BIGINT[]) AS u(provider_id,model,input_per_m,output_per_m,cached_per_m,write_per_m,image,character_per_m,second) ON CONFLICT (provider_id,model) DO UPDATE SET input_per_m_micros=excluded.input_per_m_micros,output_per_m_micros=excluded.output_per_m_micros,cached_input_per_m_micros=excluded.cached_input_per_m_micros,cache_write_per_m_micros=excluded.cache_write_per_m_micros,image_micros=excluded.image_micros,character_per_m_micros=excluded.character_per_m_micros,second_micros=excluded.second_micros,updated_at=NOW() WHERE inference_prices.source='litellm'",
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
        T2: crate::ArraySql<Item = T1>,
        T3: crate::StringSql,
        T4: crate::ArraySql<Item = T3>,
        T5: crate::ArraySql<Item = i64>,
        T6: crate::ArraySql<Item = i64>,
        T7: crate::ArraySql<Item = i64>,
        T8: crate::ArraySql<Item = i64>,
        T9: crate::ArraySql<Item = i64>,
        T10: crate::ArraySql<Item = i64>,
        T11: crate::ArraySql<Item = i64>,
    >(
        &'s self,
        client: &'c C,
        p1: &'a T2,
        p2: &'a T4,
        p3: &'a T5,
        p4: &'a T6,
        p5: &'a T7,
        p6: &'a T8,
        p7: &'a T9,
        p8: &'a T10,
        p9: &'a T11,
    ) -> Result<u64, tokio_postgres::Error> {
        client
            .execute(self.0, &[p1, p2, p3, p4, p5, p6, p7, p8, p9])
            .await
    }
}
impl<
    'a,
    C: GenericClient + Send + Sync,
    T1: crate::StringSql,
    T2: crate::ArraySql<Item = T1>,
    T3: crate::StringSql,
    T4: crate::ArraySql<Item = T3>,
    T5: crate::ArraySql<Item = i64>,
    T6: crate::ArraySql<Item = i64>,
    T7: crate::ArraySql<Item = i64>,
    T8: crate::ArraySql<Item = i64>,
    T9: crate::ArraySql<Item = i64>,
    T10: crate::ArraySql<Item = i64>,
    T11: crate::ArraySql<Item = i64>,
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
            client, &params.p1, &params.p2, &params.p3, &params.p4, &params.p5, &params.p6,
            &params.p7, &params.p8, &params.p9,
        ))
    }
}
