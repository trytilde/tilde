// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<
    T1: crate::StringSql,
    T2: crate::ArraySql<Item = T1>,
    T3: crate::StringSql,
    T4: crate::ArraySql<Item = T3>,
    T5: crate::ArraySql<Item = i64>,
    T6: crate::BytesSql,
    T7: crate::ArraySql<Item = T6>,
    T8: crate::ArraySql<Item = bool>,
    T9: crate::ArraySql<Item = bool>,
    T10: crate::StringSql,
    T11: crate::ArraySql<Item = T10>,
    T12: crate::StringSql,
    T13: crate::ArraySql<Item = T12>,
> {
    pub version_id: uuid::Uuid,
    pub paths: T2,
    pub media_types: T4,
    pub sizes: T5,
    pub digests: T7,
    pub executables: T8,
    pub inline: T9,
    pub contents: T11,
    pub object_keys: T13,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "INSERT INTO skill_files(version_id,path,media_type,size_bytes,sha256,executable,content,object_key) SELECT $1,f.path,f.media_type,f.size,f.digest,f.executable,CASE WHEN f.inline THEN f.content END,NULLIF(f.object_key,'') FROM UNNEST($2::TEXT[],$3::TEXT[],$4::BIGINT[],$5::BYTEA[],$6::BOOLEAN[],$7::BOOLEAN[],$8::TEXT[],$9::TEXT[]) AS f(path,media_type,size,digest,executable,inline,content,object_key)",
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
        T6: crate::BytesSql,
        T7: crate::ArraySql<Item = T6>,
        T8: crate::ArraySql<Item = bool>,
        T9: crate::ArraySql<Item = bool>,
        T10: crate::StringSql,
        T11: crate::ArraySql<Item = T10>,
        T12: crate::StringSql,
        T13: crate::ArraySql<Item = T12>,
    >(
        &'s self,
        client: &'c C,
        version_id: &'a uuid::Uuid,
        paths: &'a T2,
        media_types: &'a T4,
        sizes: &'a T5,
        digests: &'a T7,
        executables: &'a T8,
        inline: &'a T9,
        contents: &'a T11,
        object_keys: &'a T13,
    ) -> Result<u64, tokio_postgres::Error> {
        client
            .execute(
                self.0,
                &[
                    version_id,
                    paths,
                    media_types,
                    sizes,
                    digests,
                    executables,
                    inline,
                    contents,
                    object_keys,
                ],
            )
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
    T6: crate::BytesSql,
    T7: crate::ArraySql<Item = T6>,
    T8: crate::ArraySql<Item = bool>,
    T9: crate::ArraySql<Item = bool>,
    T10: crate::StringSql,
    T11: crate::ArraySql<Item = T10>,
    T12: crate::StringSql,
    T13: crate::ArraySql<Item = T12>,
>
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        RunParams<T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13>,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for RunStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a RunParams<T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13>,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(
            client,
            &params.version_id,
            &params.paths,
            &params.media_types,
            &params.sizes,
            &params.digests,
            &params.executables,
            &params.inline,
            &params.contents,
            &params.object_keys,
        ))
    }
}
