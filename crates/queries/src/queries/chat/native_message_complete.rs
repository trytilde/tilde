// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<
    T1: crate::StringSql,
    T2: crate::StringSql,
    T3: crate::StringSql,
    T4: crate::StringSql,
    T5: crate::StringSql,
    T6: crate::ArraySql<Item = uuid::Uuid>,
    T7: crate::ArraySql<Item = uuid::Uuid>,
> {
    pub p1: uuid::Uuid,
    pub p2: uuid::Uuid,
    pub p5: uuid::Uuid,
    pub p3: uuid::Uuid,
    pub p12: T1,
    pub p13: T2,
    pub p4: T3,
    pub p6: Option<uuid::Uuid>,
    pub p9: T4,
    pub p10: T5,
    pub p7: T6,
    pub p8: T7,
    pub p11: i64,
}
#[derive(Debug, Clone, PartialEq, Copy)]
pub struct Record {
    pub audit_sequence: i64,
    pub transaction_id: i64,
    pub target_count: i64,
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
        "WITH claimed AS ( INSERT INTO chat_tool_calls(id, thread_id, invocation_id, participant_id, name, provider_id, status, input_json, output_json) VALUES($1, $2, $3, $4, 'sendMessage', 'native', 'completed', $5, $6) ON CONFLICT(id) DO NOTHING RETURNING id ), created AS ( INSERT INTO chat_messages(id, thread_id, participant_id, text, status, invocation_id, in_reply_to_message_id, traceparent, tracestate) SELECT id, $2, $4, $7, 'complete', $3, $8, $9, $10 FROM claimed RETURNING id ), targeted AS ( INSERT INTO chat_message_targets(message_id, participant_id) SELECT created.id, p.id FROM created CROSS JOIN UNNEST($11::UUID[]) AS recipient JOIN chat_participants p ON p.id = recipient AND p.thread_id = $2 AND p.active RETURNING participant_id ), attached AS ( INSERT INTO chat_message_attachments(thread_id, message_id, attachment_id) SELECT $2, created.id, attachment FROM created CROSS JOIN UNNEST($12::UUID[]) AS attachment RETURNING attachment_id ), advanced AS ( UPDATE chat_threads SET activity_sequence = activity_sequence + $13::BIGINT WHERE id = $2 AND EXISTS(SELECT 1 FROM created) RETURNING activity_sequence - $13::BIGINT + 1 AS sequence ) SELECT advanced.sequence AS audit_sequence, txid_current() AS transaction_id, (SELECT COUNT(*) FROM targeted) AS target_count FROM advanced",
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
        T6: crate::ArraySql<Item = uuid::Uuid>,
        T7: crate::ArraySql<Item = uuid::Uuid>,
    >(
        &'s self,
        client: &'c C,
        p1: &'a uuid::Uuid,
        p2: &'a uuid::Uuid,
        p5: &'a uuid::Uuid,
        p3: &'a uuid::Uuid,
        p12: &'a T1,
        p13: &'a T2,
        p4: &'a T3,
        p6: &'a Option<uuid::Uuid>,
        p9: &'a T4,
        p10: &'a T5,
        p7: &'a T6,
        p8: &'a T7,
        p11: &'a i64,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 13> {
        RecordQuery {
            client,
            params: [p1, p2, p5, p3, p12, p13, p4, p6, p9, p10, p7, p8, p11],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row: &tokio_postgres::Row| -> Result<Record, tokio_postgres::Error> {
                Ok(Record {
                    audit_sequence: row.try_get(0)?,
                    transaction_id: row.try_get(1)?,
                    target_count: row.try_get(2)?,
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
    T6: crate::ArraySql<Item = uuid::Uuid>,
    T7: crate::ArraySql<Item = uuid::Uuid>,
>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        RunParams<T1, T2, T3, T4, T5, T6, T7>,
        RecordQuery<'c, 'a, 's, C, Record, 13>,
        C,
    > for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1, T2, T3, T4, T5, T6, T7>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 13> {
        self.bind(
            client,
            &params.p1,
            &params.p2,
            &params.p5,
            &params.p3,
            &params.p12,
            &params.p13,
            &params.p4,
            &params.p6,
            &params.p9,
            &params.p10,
            &params.p7,
            &params.p8,
            &params.p11,
        )
    }
}
