// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::StringSql, T2: crate::StringSql> {
    pub role_id: T1,
    pub group_id: Option<T2>,
    pub user_id: Option<uuid::Uuid>,
    pub api_key_id: Option<uuid::Uuid>,
    pub by_user: Option<uuid::Uuid>,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct StringQuery<'c, 'a, 's, C: GenericClient, T, const N: usize> {
    client: &'c C,
    params: [&'a (dyn postgres_types::ToSql + Sync); N],
    query: &'static str,
    cached: Option<&'s tokio_postgres::Statement>,
    extractor: fn(&tokio_postgres::Row) -> Result<&str, tokio_postgres::Error>,
    mapper: fn(&str) -> T,
}
impl<'c, 'a, 's, C, T: 'c, const N: usize> StringQuery<'c, 'a, 's, C, T, N>
where
    C: GenericClient,
{
    pub fn map<R>(self, mapper: fn(&str) -> R) -> StringQuery<'c, 'a, 's, C, R, N> {
        StringQuery {
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
        "INSERT INTO iam_role_members(role_id,group_id,user_id,api_key_id,granted_by_user_id) SELECT $1,$2,$3,$4,$5 WHERE ($2::TEXT IS NULL OR EXISTS(SELECT 1 FROM iam_groups WHERE id=$2)) AND ($3::UUID IS NULL OR EXISTS(SELECT 1 FROM iam_users WHERE id=$3)) AND ($4::UUID IS NULL OR EXISTS(SELECT 1 FROM iam_api_keys WHERE id=$4 AND revoked_at IS NULL)) ON CONFLICT(role_id,principal) DO UPDATE SET granted_by_user_id=iam_role_members.granted_by_user_id RETURNING principal",
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
    pub fn bind<'c, 'a, 's, C: GenericClient, T1: crate::StringSql, T2: crate::StringSql>(
        &'s self,
        client: &'c C,
        role_id: &'a T1,
        group_id: &'a Option<T2>,
        user_id: &'a Option<uuid::Uuid>,
        api_key_id: &'a Option<uuid::Uuid>,
        by_user: &'a Option<uuid::Uuid>,
    ) -> StringQuery<'c, 'a, 's, C, String, 5> {
        StringQuery {
            client,
            params: [role_id, group_id, user_id, api_key_id, by_user],
            query: self.0,
            cached: self.1.as_ref(),
            extractor: |row| Ok(row.try_get(0)?),
            mapper: |it| it.into(),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::StringSql, T2: crate::StringSql>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        RunParams<T1, T2>,
        StringQuery<'c, 'a, 's, C, String, 5>,
        C,
    > for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1, T2>,
    ) -> StringQuery<'c, 'a, 's, C, String, 5> {
        self.bind(
            client,
            &params.role_id,
            &params.group_id,
            &params.user_id,
            &params.api_key_id,
            &params.by_user,
        )
    }
}
