// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub bank_id: uuid::Uuid,
    pub connection_id: uuid::Uuid,
    pub model: String,
    pub instructions: String,
    pub attempts: i32,
    pub scope: String,
    pub name: String,
    pub description: String,
}
pub struct RecordBorrowed<'a> {
    pub bank_id: uuid::Uuid,
    pub connection_id: uuid::Uuid,
    pub model: &'a str,
    pub instructions: &'a str,
    pub attempts: i32,
    pub scope: &'a str,
    pub name: &'a str,
    pub description: &'a str,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            bank_id,
            connection_id,
            model,
            instructions,
            attempts,
            scope,
            name,
            description,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            bank_id,
            connection_id,
            model: model.into(),
            instructions: instructions.into(),
            attempts,
            scope: scope.into(),
            name: name.into(),
            description: description.into(),
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
        "UPDATE memory_synthesis s SET lease_until=NOW()+make_interval(secs=>$1::DOUBLE PRECISION) FROM memory_banks b WHERE b.id=s.bank_id AND s.bank_id=( SELECT bank_id FROM memory_synthesis WHERE run_after<=NOW() AND lease_until<=NOW() ORDER BY run_after LIMIT 1 FOR UPDATE SKIP LOCKED) RETURNING s.bank_id,s.connection_id,s.model,s.instructions,s.attempts,b.scope,b.name,b.description",
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
        p1: &'a f64,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 1> {
        RecordQuery {
            client,
            params: [p1],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        bank_id: row.try_get(0)?,
                        connection_id: row.try_get(1)?,
                        model: row.try_get(2)?,
                        instructions: row.try_get(3)?,
                        attempts: row.try_get(4)?,
                        scope: row.try_get(5)?,
                        name: row.try_get(6)?,
                        description: row.try_get(7)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
