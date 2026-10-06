// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::StringSql> {
    pub p1: Option<uuid::Uuid>,
    pub p2: Option<T1>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub name: String,
    pub connection_id: uuid::Uuid,
    pub template: String,
    pub reuse: String,
    pub sleep_after_secs: i32,
    pub terminate_after_secs: i32,
    pub connect_timeout_secs: i32,
    pub env_names: Vec<String>,
    pub agent_ids: Vec<uuid::Uuid>,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub name: &'a str,
    pub connection_id: uuid::Uuid,
    pub template: &'a str,
    pub reuse: &'a str,
    pub sleep_after_secs: i32,
    pub terminate_after_secs: i32,
    pub connect_timeout_secs: i32,
    pub env_names: crate::ArrayIterator<'a, &'a str>,
    pub agent_ids: crate::ArrayIterator<'a, uuid::Uuid>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            name,
            connection_id,
            template,
            reuse,
            sleep_after_secs,
            terminate_after_secs,
            connect_timeout_secs,
            env_names,
            agent_ids,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            connection_id,
            template: template.into(),
            reuse: reuse.into(),
            sleep_after_secs,
            terminate_after_secs,
            connect_timeout_secs,
            env_names: env_names.map(|v| v.into()).collect(),
            agent_ids: agent_ids.map(|v| v).collect(),
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
        "SELECT b.id,b.name,b.connection_id,b.template,b.reuse,b.sleep_after_secs,b.terminate_after_secs,b.connect_timeout_secs, COALESCE((SELECT array_agg(e.name ORDER BY e.name) FROM sandbox_blueprint_env e WHERE e.blueprint_id=b.id), '{}')::TEXT[] AS env_names, COALESCE((SELECT array_agg(a.agent_id ORDER BY a.agent_id) FROM agent_sandboxes a WHERE a.blueprint_id=b.id), '{}')::UUID[] AS agent_ids FROM sandbox_blueprints b WHERE ($1::UUID IS NULL OR b.id=$1) AND ($2::TEXT IS NULL OR b.name ILIKE '%'||$2||'%') ORDER BY b.name",
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
    pub fn bind<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>(
        &'s self,
        client: &'c C,
        p1: &'a Option<uuid::Uuid>,
        p2: &'a Option<T1>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        RecordQuery {
            client,
            params: [p1, p2],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        id: row.try_get(0)?,
                        name: row.try_get(1)?,
                        connection_id: row.try_get(2)?,
                        template: row.try_get(3)?,
                        reuse: row.try_get(4)?,
                        sleep_after_secs: row.try_get(5)?,
                        terminate_after_secs: row.try_get(6)?,
                        connect_timeout_secs: row.try_get(7)?,
                        env_names: row.try_get(8)?,
                        agent_ids: row.try_get(9)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::StringSql>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        RunParams<T1>,
        RecordQuery<'c, 'a, 's, C, Record, 2>,
        C,
    > for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        self.bind(client, &params.p1, &params.p2)
    }
}
