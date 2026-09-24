// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<
    T1: crate::ArraySql<Item = i64>,
    T2: crate::StringSql,
    T3: crate::ArraySql<Item = T2>,
    T4: crate::ArraySql<Item = uuid::Uuid>,
    T5: crate::StringSql,
    T6: crate::ArraySql<Item = T5>,
    T7: crate::BytesSql,
    T8: crate::ArraySql<Item = T7>,
    T9: crate::ArraySql<Item = chrono::DateTime<chrono::Utc>>,
    T10: crate::ArraySql<Item = bool>,
> {
    pub p2: T1,
    pub p3: T3,
    pub p4: T4,
    pub p5: T6,
    pub p6: T8,
    pub p7: T9,
    pub p8: T10,
    pub p1: uuid::Uuid,
}
#[derive(Debug, Clone, PartialEq, Copy)]
pub struct Record {
    pub sequence: i64,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RecordQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<Record, tokio_postgres::Error>,
    mapper: fn(Record) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> RecordQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(self, mapper: fn(Record) -> R) -> RecordQuery<'c, 'a, 's, C, R, N> {
        RecordQuery {
            client: self.client,
            params: self.params,
            query: self.query,
            cached: self.cached,
            extractor: self.extractor,
            mapper,
        }
    }
    pub async fn one(self) -> Result<T, tokio_postgres::Error> {
        let row =
            crate::client::async_::one(self.client, self.query, &self.params, self.cached).await?;
        Ok((self.mapper)((self.extractor)(&row)?))
    }
    pub async fn all(self) -> Result<Vec<T>, tokio_postgres::Error> {
        self.iter().await?.try_collect().await
    }
    pub async fn opt(self) -> Result<Option<T>, tokio_postgres::Error> {
        let opt_row =
            crate::client::async_::opt(self.client, self.query, &self.params, self.cached).await?;
        Ok(opt_row
            .map(|row| {
                let extracted = (self.extractor)(&row)?;
                Ok((self.mapper)(extracted))
            })
            .transpose()?)
    }
    pub async fn iter(
        self,
    ) -> Result<
        impl futures::Stream<Item = Result<T, tokio_postgres::Error>> + 'c,
        tokio_postgres::Error,
    > {
        let stream = crate::client::async_::raw(
            self.client,
            self.query,
            crate::slice_iter(&self.params),
            self.cached,
        )
        .await?;
        let mapped = stream
            .map(move |res| {
                res.and_then(|row| {
                    let extracted = (self.extractor)(&row)?;
                    Ok((self.mapper)(extracted))
                })
            })
            .into_stream();
        Ok(mapped)
    }
}
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "WITH input AS MATERIALIZED ( SELECT * FROM UNNEST($1::BIGINT[], $2::TEXT[], $3::UUID[], $4::TEXT[], $5::BYTEA[], $6::TIMESTAMPTZ[], $7::BOOLEAN[]) AS v(sequence, kind, entity_id, text_delta, snapshot, created_at, skip_gap) ), watermark AS ( SELECT COALESCE(MAX(sequence), 0) AS last FROM chat_activity WHERE thread_id = $8 ), ordered AS ( SELECT i.*, ROW_NUMBER() OVER (ORDER BY i.sequence) AS position, FIRST_VALUE(i.skip_gap) OVER (ORDER BY i.sequence) AS allow_gap, w.last FROM input i CROSS JOIN watermark w WHERE i.sequence > w.last ), written AS ( INSERT INTO chat_activity(thread_id, sequence, kind, entity_id, text_delta, snapshot, created_at) SELECT $8, sequence, kind, entity_id, text_delta, snapshot, created_at FROM ordered WHERE (sequence = last + position OR allow_gap) AND EXISTS(SELECT 1 FROM chat_threads WHERE id = $8) ORDER BY sequence ON CONFLICT(thread_id, sequence) DO NOTHING RETURNING sequence ) SELECT i.sequence AS sequence FROM input i CROSS JOIN watermark w WHERE i.sequence <= w.last OR i.sequence IN (SELECT sequence FROM written) OR NOT EXISTS(SELECT 1 FROM chat_threads WHERE id = $8)",
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
    pub fn bind<
        'c,
        'a,
        's,
        C: GenericClient,
        T1: crate::ArraySql<Item = i64>,
        T2: crate::StringSql,
        T3: crate::ArraySql<Item = T2>,
        T4: crate::ArraySql<Item = uuid::Uuid>,
        T5: crate::StringSql,
        T6: crate::ArraySql<Item = T5>,
        T7: crate::BytesSql,
        T8: crate::ArraySql<Item = T7>,
        T9: crate::ArraySql<Item = chrono::DateTime<chrono::Utc>>,
        T10: crate::ArraySql<Item = bool>,
    >(
        &'s self,
        client: &'c C,
        p2: &'a T1,
        p3: &'a T3,
        p4: &'a T4,
        p5: &'a T6,
        p6: &'a T8,
        p7: &'a T9,
        p8: &'a T10,
        p1: &'a uuid::Uuid,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 8> {
        RecordQuery {
            client,
            params: [p2, p3, p4, p5, p6, p7, p8, p1],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row: &tokio_postgres::Row| -> Result<Record, tokio_postgres::Error> {
                Ok(Record {
                    sequence: row.try_get(0)?,
                })
            },
            mapper: |it| Record::from(it),
        }
    }
}
impl<
    'c,
    'a,
    's,
    C: GenericClient,
    T1: crate::ArraySql<Item = i64>,
    T2: crate::StringSql,
    T3: crate::ArraySql<Item = T2>,
    T4: crate::ArraySql<Item = uuid::Uuid>,
    T5: crate::StringSql,
    T6: crate::ArraySql<Item = T5>,
    T7: crate::BytesSql,
    T8: crate::ArraySql<Item = T7>,
    T9: crate::ArraySql<Item = chrono::DateTime<chrono::Utc>>,
    T10: crate::ArraySql<Item = bool>,
>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        RunParams<T1, T2, T3, T4, T5, T6, T7, T8, T9, T10>,
        RecordQuery<'c, 'a, 's, C, Record, 8>,
        C,
    > for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1, T2, T3, T4, T5, T6, T7, T8, T9, T10>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 8> {
        self.bind(
            client, &params.p2, &params.p3, &params.p4, &params.p5, &params.p6, &params.p7,
            &params.p8, &params.p1,
        )
    }
}
