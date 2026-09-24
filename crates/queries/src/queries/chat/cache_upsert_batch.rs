// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<
    T1: crate::ArraySql<Item = uuid::Uuid>,
    T2: crate::StringSql,
    T3: crate::ArraySql<Item = T2>,
> {
    pub p3: T1,
    pub p4: T3,
    pub p2: uuid::Uuid,
    pub p1: uuid::Uuid,
}
#[derive(Debug, Clone, PartialEq, Copy)]
pub struct Record {
    pub missing: i64,
    pub incomplete: i64,
    pub written: i64,
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
        "WITH input AS MATERIALIZED ( SELECT * FROM UNNEST($1::UUID[], $2::TEXT[]) AS v(message_id, representation) ), checked AS MATERIALIZED ( SELECT i.*, m.id IS NOT NULL AS present, m.status = 'complete' AS complete FROM input i LEFT JOIN chat_messages m ON m.id = i.message_id AND m.thread_id = $3 ), validation AS ( SELECT COUNT(*) FILTER (WHERE NOT present) AS missing, COUNT(*) FILTER (WHERE present AND NOT complete) AS incomplete FROM checked ), written AS ( INSERT INTO chat_converted_messages(agent_id, message_id, representation) SELECT $4, message_id, representation::JSONB FROM checked WHERE (SELECT missing = 0 AND incomplete = 0 FROM validation) ON CONFLICT(agent_id, message_id) DO UPDATE SET representation = EXCLUDED.representation RETURNING message_id ) SELECT missing AS missing, incomplete AS incomplete, (SELECT COUNT(*) FROM written) AS written FROM validation",
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
        T1: crate::ArraySql<Item = uuid::Uuid>,
        T2: crate::StringSql,
        T3: crate::ArraySql<Item = T2>,
    >(
        &'s self,
        client: &'c C,
        p3: &'a T1,
        p4: &'a T3,
        p2: &'a uuid::Uuid,
        p1: &'a uuid::Uuid,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 4> {
        RecordQuery {
            client,
            params: [p3, p4, p2, p1],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row: &tokio_postgres::Row| -> Result<Record, tokio_postgres::Error> {
                Ok(Record {
                    missing: row.try_get(0)?,
                    incomplete: row.try_get(1)?,
                    written: row.try_get(2)?,
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
    T1: crate::ArraySql<Item = uuid::Uuid>,
    T2: crate::StringSql,
    T3: crate::ArraySql<Item = T2>,
>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        RunParams<T1, T2, T3>,
        RecordQuery<'c, 'a, 's, C, Record, 4>,
        C,
    > for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1, T2, T3>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 4> {
        self.bind(client, &params.p3, &params.p4, &params.p2, &params.p1)
    }
}
