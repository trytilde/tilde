// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<
    T1: crate::ArraySql<Item = uuid::Uuid>,
    T2: crate::StringSql,
    T3: crate::StringSql,
    T4: crate::StringSql,
> {
    pub pb: T1,
    pub pn: i32,
    pub pk: Option<T2>,
    pub pp: Option<T3>,
    pub pf: Option<chrono::DateTime<chrono::Utc>>,
    pub pt: Option<chrono::DateTime<chrono::Utc>>,
    pub pe: bool,
    pub pi: Option<uuid::Uuid>,
    pub pv: T4,
    pub pl: i64,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct UuidUuidQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<uuid::Uuid, tokio_postgres::Error>,
    mapper: fn(uuid::Uuid) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> UuidUuidQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(self, mapper: fn(uuid::Uuid) -> R) -> UuidUuidQuery<'c, 'a, 's, C, R, N> {
        UuidUuidQuery {
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
        "SELECT m.id FROM memories m JOIN memory_banks b ON b.id=m.bank_id WHERE m.bank_id=ANY($1::UUID[]) AND m.deleted_at IS NULL AND m.embedding_dims=1536 AND m.embedding_version=$2 AND ($3::TEXT IS NULL OR m.kind=$3) AND ($4::TEXT IS NULL OR $4='' OR m.path=$4 OR starts_with(m.path,$4||'/')) AND (($5::TIMESTAMPTZ IS NULL AND $6::TIMESTAMPTZ IS NULL) OR m.valid_during && tstzrange($5,$6,'[]')) AND ($7 OR b.scope='shared' OR m.identity_id IS NULL OR m.identity_id=$8::UUID) AND NOT EXISTS (SELECT 1 FROM memories s WHERE s.supersedes_id=m.id) ORDER BY m.embedding::halfvec(1536) <=> $9::TEXT::halfvec(1536) LIMIT $10",
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
        T3: crate::StringSql,
        T4: crate::StringSql,
    >(
        &'s self,
        client: &'c C,
        pb: &'a T1,
        pn: &'a i32,
        pk: &'a Option<T2>,
        pp: &'a Option<T3>,
        pf: &'a Option<chrono::DateTime<chrono::Utc>>,
        pt: &'a Option<chrono::DateTime<chrono::Utc>>,
        pe: &'a bool,
        pi: &'a Option<uuid::Uuid>,
        pv: &'a T4,
        pl: &'a i64,
    ) -> UuidUuidQuery<'c, 'a, 's, C, uuid::Uuid, 10> {
        UuidUuidQuery {
            client,
            params: [pb, pn, pk, pp, pf, pt, pe, pi, pv, pl],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row| Ok(row.try_get(0)?),
            mapper: |it| it,
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
    T3: crate::StringSql,
    T4: crate::StringSql,
>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        RunParams<T1, T2, T3, T4>,
        UuidUuidQuery<'c, 'a, 's, C, uuid::Uuid, 10>,
        C,
    > for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1, T2, T3, T4>,
    ) -> UuidUuidQuery<'c, 'a, 's, C, uuid::Uuid, 10> {
        self.bind(
            client, &params.pb, &params.pn, &params.pk, &params.pp, &params.pf, &params.pt,
            &params.pe, &params.pi, &params.pv, &params.pl,
        )
    }
}
