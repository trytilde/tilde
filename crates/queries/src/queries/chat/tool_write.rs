// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<
    T1: crate::StringSql,
    T2: crate::StringSql,
    T3: crate::StringSql,
    T4: crate::StringSql,
    T5: crate::StringSql,
    T6: crate::StringSql,
    T7: crate::StringSql,
    T8: crate::StringSql,
    T9: crate::StringSql,
> {
    pub p1: uuid::Uuid,
    pub p2: uuid::Uuid,
    pub p3: uuid::Uuid,
    pub p4: uuid::Uuid,
    pub p5: T1,
    pub p6: T2,
    pub p8: T3,
    pub p13: T4,
    pub p14: bool,
    pub p15: T5,
    pub p11: T6,
    pub p9: T7,
    pub p7: T8,
    pub p10: T9,
    pub p12: i64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub existed: bool,
    pub applied: bool,
    pub replayed: bool,
    pub transaction_id: i64,
    pub display: String,
}
pub struct RecordBorrowed<'a> {
    pub existed: bool,
    pub applied: bool,
    pub replayed: bool,
    pub transaction_id: i64,
    pub display: &'a str,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            existed,
            applied,
            replayed,
            transaction_id,
            display,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            existed,
            applied,
            replayed,
            transaction_id,
            display: display.into(),
        }
    }
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RecordQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<RecordBorrowed, tokio_postgres::Error>,
    mapper: fn(RecordBorrowed) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> RecordQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(self, mapper: fn(RecordBorrowed) -> R) -> RecordQuery<'c, 'a, 's, C, R, N> {
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
        "WITH existing AS MATERIALIZED ( SELECT * FROM chat_tool_calls WHERE id = $1 ), matched AS MATERIALIZED ( SELECT * FROM existing WHERE thread_id = $2 AND invocation_id = $3 AND participant_id = $4 AND name = $5 AND provider_id = $6 AND ($7 = '' OR input_json = $7) ), started AS ( INSERT INTO chat_tool_calls(id, thread_id, invocation_id, participant_id, name, provider_id, status, input_json, summary, detached, display) SELECT $1, $2, $3, $4, $5, $6, 'running', $7, $8, $9, $10 WHERE $11 = 'tool.started' AND $7 <> '' AND $12 = '' ON CONFLICT(id) DO NOTHING RETURNING id ), finished AS ( UPDATE chat_tool_calls SET status = $13, output_json = $12, error = $14, updated_at = NOW() WHERE id IN (SELECT id FROM matched WHERE status = 'running') AND $11 IN ('tool.completed', 'tool.failed', 'tool.aborted') RETURNING id ), accepted AS ( SELECT 1 WHERE EXISTS(SELECT 1 FROM started) OR EXISTS(SELECT 1 FROM finished) OR ($11 = 'tool.input.delta' AND EXISTS(SELECT 1 FROM matched WHERE status = 'running')) ), advanced AS ( UPDATE chat_threads SET activity_sequence = $15::BIGINT WHERE id = $2 AND activity_sequence = $15::BIGINT - 1 AND EXISTS(SELECT 1 FROM accepted) RETURNING id ) SELECT EXISTS(SELECT 1 FROM existing) AS existed, EXISTS(SELECT 1 FROM advanced) AS applied, EXISTS(SELECT 1 FROM matched WHERE status <> 'running' AND status = $13 AND output_json = $12 AND error = $14) AS replayed, txid_current() AS transaction_id, COALESCE((SELECT display FROM existing), $10) AS display",
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
        T1: crate::StringSql,
        T2: crate::StringSql,
        T3: crate::StringSql,
        T4: crate::StringSql,
        T5: crate::StringSql,
        T6: crate::StringSql,
        T7: crate::StringSql,
        T8: crate::StringSql,
        T9: crate::StringSql,
    >(
        &'s self,
        client: &'c C,
        p1: &'a uuid::Uuid,
        p2: &'a uuid::Uuid,
        p3: &'a uuid::Uuid,
        p4: &'a uuid::Uuid,
        p5: &'a T1,
        p6: &'a T2,
        p8: &'a T3,
        p13: &'a T4,
        p14: &'a bool,
        p15: &'a T5,
        p11: &'a T6,
        p9: &'a T7,
        p7: &'a T8,
        p10: &'a T9,
        p12: &'a i64,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 15> {
        RecordQuery {
            client,
            params: [
                p1, p2, p3, p4, p5, p6, p8, p13, p14, p15, p11, p9, p7, p10, p12,
            ],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        existed: row.try_get(0)?,
                        applied: row.try_get(1)?,
                        replayed: row.try_get(2)?,
                        transaction_id: row.try_get(3)?,
                        display: row.try_get(4)?,
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
    T1: crate::StringSql,
    T2: crate::StringSql,
    T3: crate::StringSql,
    T4: crate::StringSql,
    T5: crate::StringSql,
    T6: crate::StringSql,
    T7: crate::StringSql,
    T8: crate::StringSql,
    T9: crate::StringSql,
>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        RunParams<T1, T2, T3, T4, T5, T6, T7, T8, T9>,
        RecordQuery<'c, 'a, 's, C, Record, 15>,
        C,
    > for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1, T2, T3, T4, T5, T6, T7, T8, T9>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 15> {
        self.bind(
            client,
            &params.p1,
            &params.p2,
            &params.p3,
            &params.p4,
            &params.p5,
            &params.p6,
            &params.p8,
            &params.p13,
            &params.p14,
            &params.p15,
            &params.p11,
            &params.p9,
            &params.p7,
            &params.p10,
            &params.p12,
        )
    }
}
