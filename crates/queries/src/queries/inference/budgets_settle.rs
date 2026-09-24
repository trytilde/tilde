// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub scope: String,
    pub scope_id: uuid::Uuid,
    pub connection_id: Option<uuid::Uuid>,
    pub exhausted: bool,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub scope: &'a str,
    pub scope_id: uuid::Uuid,
    pub connection_id: Option<uuid::Uuid>,
    pub exhausted: bool,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            scope,
            scope_id,
            connection_id,
            exhausted,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            scope: scope.into(),
            scope_id,
            connection_id,
            exhausted,
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
        "WITH spend AS ( SELECT b.id, COALESCE((SELECT SUM(r.cost_micros) FROM inference_requests r LEFT JOIN chat_runs cr ON b.scope='identity' AND cr.id=r.run_id LEFT JOIN chat_users u ON u.id=cr.source_identity_id WHERE r.cost_micros IS NOT NULL AND ((b.scope='agent' AND r.agent_id=b.scope_id) OR (b.scope='identity' AND u.root_identity_id=b.scope_id)) AND (b.connection_id IS NULL OR r.connection_id=b.connection_id) AND r.created_at >= CASE b.period WHEN 'day' THEN date_trunc('day',NOW()) WHEN 'month' THEN date_trunc('month',NOW()) ELSE '-infinity'::TIMESTAMPTZ END),0)::BIGINT AS spent, CASE b.period WHEN 'day' THEN date_trunc('day',NOW())+INTERVAL '1 day' WHEN 'month' THEN date_trunc('month',NOW())+INTERVAL '1 month' ELSE '9999-12-31T00:00:00Z'::TIMESTAMPTZ END AS period_end FROM inference_budgets b ) UPDATE inference_budgets b SET spent_micros=s.spent, exhausted_until=CASE WHEN b.action='block' AND s.spent>=b.limit_micros THEN s.period_end ELSE NULL END, updated_at=CASE WHEN b.spent_micros<>s.spent OR (b.exhausted_until IS NOT NULL)<>(b.action='block' AND s.spent>=b.limit_micros) THEN NOW() ELSE b.updated_at END FROM spend s WHERE s.id=b.id RETURNING b.id,b.scope,b.scope_id,b.connection_id,(b.exhausted_until IS NOT NULL AND b.exhausted_until>NOW()) AS exhausted",
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
    pub fn bind<'c, 'a, 's, C: GenericClient>(
        &'s self,
        client: &'c C,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 0> {
        RecordQuery {
            client,
            params: [],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        id: row.try_get(0)?,
                        scope: row.try_get(1)?,
                        scope_id: row.try_get(2)?,
                        connection_id: row.try_get(3)?,
                        exhausted: row.try_get(4)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
