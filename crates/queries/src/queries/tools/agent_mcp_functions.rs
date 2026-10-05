// This file was generated with `cornucopia`. Do not modify.

#[derive(Clone, Copy, Debug)]
pub struct RunParams {
    pub p1: Option<uuid::Uuid>,
    pub p2: Option<uuid::Uuid>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub slug: String,
    pub name: String,
    pub tool_name: String,
    pub is_async: bool,
    pub summary: String,
    pub description: String,
    pub display: String,
    pub connection_id: uuid::Uuid,
    pub provider_id: String,
    pub type_id: String,
    pub tool_description: String,
    pub input_schema_json: String,
    pub output_schema_json: String,
    pub read_only: bool,
    pub destructive: bool,
    pub idempotent: bool,
    pub open_world: bool,
}
pub struct RecordBorrowed<'a> {
    pub slug: &'a str,
    pub name: &'a str,
    pub tool_name: &'a str,
    pub is_async: bool,
    pub summary: &'a str,
    pub description: &'a str,
    pub display: &'a str,
    pub connection_id: uuid::Uuid,
    pub provider_id: &'a str,
    pub type_id: &'a str,
    pub tool_description: &'a str,
    pub input_schema_json: &'a str,
    pub output_schema_json: &'a str,
    pub read_only: bool,
    pub destructive: bool,
    pub idempotent: bool,
    pub open_world: bool,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            slug,
            name,
            tool_name,
            is_async,
            summary,
            description,
            display,
            connection_id,
            provider_id,
            type_id,
            tool_description,
            input_schema_json,
            output_schema_json,
            read_only,
            destructive,
            idempotent,
            open_world,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            slug: slug.into(),
            name: name.into(),
            tool_name: tool_name.into(),
            is_async,
            summary: summary.into(),
            description: description.into(),
            display: display.into(),
            connection_id,
            provider_id: provider_id.into(),
            type_id: type_id.into(),
            tool_description: tool_description.into(),
            input_schema_json: input_schema_json.into(),
            output_schema_json: output_schema_json.into(),
            read_only,
            destructive,
            idempotent,
            open_world,
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
        "SELECT s.slug,a.name,a.tool_name,a.is_async,a.summary,a.description,a.display,c.id AS connection_id,c.provider_id,c.type_id, t.description AS tool_description,t.input_schema_json,t.output_schema_json, t.read_only,t.destructive,t.idempotent,t.open_world FROM agent_tool_sources s JOIN agent_tools a ON a.source_id=s.id JOIN connections c ON c.id=s.connection_id AND c.status='ready' JOIN connection_types ct ON ct.provider_id=c.provider_id AND ct.type_id=c.type_id AND ct.mcp_credential IS NOT NULL JOIN connection_tools t ON t.connection_id=c.id AND t.name=a.tool_name WHERE (s.agent_id=$1 OR s.sandbox_blueprint_id=$2) ORDER BY s.slug,a.name",
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
        p1: &'a Option<uuid::Uuid>,
        p2: &'a Option<uuid::Uuid>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        RecordQuery {
            client,
            params: [p1, p2],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        slug: row.try_get(0)?,
                        name: row.try_get(1)?,
                        tool_name: row.try_get(2)?,
                        is_async: row.try_get(3)?,
                        summary: row.try_get(4)?,
                        description: row.try_get(5)?,
                        display: row.try_get(6)?,
                        connection_id: row.try_get(7)?,
                        provider_id: row.try_get(8)?,
                        type_id: row.try_get(9)?,
                        tool_description: row.try_get(10)?,
                        input_schema_json: row.try_get(11)?,
                        output_schema_json: row.try_get(12)?,
                        read_only: row.try_get(13)?,
                        destructive: row.try_get(14)?,
                        idempotent: row.try_get(15)?,
                        open_world: row.try_get(16)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient>
    crate::client::async_::Params<'c, 'a, 's, RunParams, RecordQuery<'c, 'a, 's, C, Record, 2>, C>
    for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 2> {
        self.bind(client, &params.p1, &params.p2)
    }
}
