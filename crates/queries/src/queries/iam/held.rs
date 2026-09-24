// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<
    T1: crate::StringSql,
    T2: crate::StringSql,
    T3: crate::StringSql,
    T4: crate::ArraySql<Item = T3>,
    T5: crate::ArraySql<Item = uuid::Uuid>,
> {
    pub kind: T1,
    pub action: T2,
    pub is_admin: bool,
    pub caller_user: Option<uuid::Uuid>,
    pub caller_key: Option<uuid::Uuid>,
    pub caller_groups: T4,
    pub ids: T5,
}
#[derive(Debug, Clone, PartialEq, Copy)]
pub struct Record {
    pub id: uuid::Uuid,
    pub held: bool,
    pub visible: bool,
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
        "SELECT r.id, iam_held($1,r.id,ARRAY[$2]::TEXT[],$3,$4,$5,$6::TEXT[]) AS held, iam_held($1,r.id,ARRAY['view']::TEXT[],$3,$4,$5,$6::TEXT[]) AS visible FROM unnest($7::UUID[]) AS r(id)",
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
        T4: crate::ArraySql<Item = T3>,
        T5: crate::ArraySql<Item = uuid::Uuid>,
    >(
        &'s self,
        client: &'c C,
        kind: &'a T1,
        action: &'a T2,
        is_admin: &'a bool,
        caller_user: &'a Option<uuid::Uuid>,
        caller_key: &'a Option<uuid::Uuid>,
        caller_groups: &'a T4,
        ids: &'a T5,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 7> {
        RecordQuery {
            client,
            params: [
                kind,
                action,
                is_admin,
                caller_user,
                caller_key,
                caller_groups,
                ids,
            ],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row: &tokio_postgres::Row| -> Result<Record, tokio_postgres::Error> {
                Ok(Record {
                    id: row.try_get(0)?,
                    held: row.try_get(1)?,
                    visible: row.try_get(2)?,
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
    T4: crate::ArraySql<Item = T3>,
    T5: crate::ArraySql<Item = uuid::Uuid>,
>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        RunParams<T1, T2, T3, T4, T5>,
        RecordQuery<'c, 'a, 's, C, Record, 7>,
        C,
    > for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1, T2, T3, T4, T5>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 7> {
        self.bind(
            client,
            &params.kind,
            &params.action,
            &params.is_admin,
            &params.caller_user,
            &params.caller_key,
            &params.caller_groups,
            &params.ids,
        )
    }
}
