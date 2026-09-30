// This file was generated with `cornucopia`. Do not modify.

#[derive(Clone, Copy, Debug)]
pub struct RunParams {
    pub p1: uuid::Uuid,
    pub p2: f64,
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
    pub tool_host_id: uuid::Uuid,
    pub execution_type: String,
    pub function_arn: Option<String>,
    pub connection_id: Option<uuid::Uuid>,
    pub connection_type: Option<String>,
    pub tool_description: String,
    pub tool_summary: String,
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
    pub tool_host_id: uuid::Uuid,
    pub execution_type: &'a str,
    pub function_arn: Option<&'a str>,
    pub connection_id: Option<uuid::Uuid>,
    pub connection_type: Option<&'a str>,
    pub tool_description: &'a str,
    pub tool_summary: &'a str,
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
            tool_host_id,
            execution_type,
            function_arn,
            connection_id,
            connection_type,
            tool_description,
            tool_summary,
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
            tool_host_id,
            execution_type: execution_type.into(),
            function_arn: function_arn.map(|v| v.into()),
            connection_id,
            connection_type: connection_type.map(|v| v.into()),
            tool_description: tool_description.into(),
            tool_summary: tool_summary.into(),
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
        "SELECT s.slug,a.name,a.tool_name,a.is_async,a.summary,a.description,a.display,h.id AS tool_host_id,h.execution_type,h.function_arn, c.id AS connection_id,c.type_id AS connection_type, t.description AS tool_description,t.summary AS tool_summary,t.input_schema_json,t.output_schema_json, t.read_only,t.destructive,t.idempotent,t.open_world FROM agent_tool_sources s JOIN agent_tools a ON a.source_id=s.id LEFT JOIN connections c ON c.id=s.connection_id LEFT JOIN connection_providers p ON p.provider_id=c.provider_id JOIN tool_hosts h ON h.id=COALESCE(s.tool_host_id,p.tool_host_id) JOIN tool_host_tools t ON t.tool_host_id=h.id AND t.name=a.tool_name WHERE s.agent_id=$1 AND (h.execution_type='lambda' OR h.connected_at > NOW() - make_interval(secs => $2)) AND CASE WHEN s.tool_host_id IS NOT NULL THEN NOT EXISTS (SELECT 1 FROM connection_providers hp WHERE hp.tool_host_id=h.id) ELSE c.status='ready' AND c.owner_user_id IS NULL END ORDER BY s.slug,a.name",
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
        p1: &'a uuid::Uuid,
        p2: &'a f64,
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
                        tool_host_id: row.try_get(7)?,
                        execution_type: row.try_get(8)?,
                        function_arn: row.try_get(9)?,
                        connection_id: row.try_get(10)?,
                        connection_type: row.try_get(11)?,
                        tool_description: row.try_get(12)?,
                        tool_summary: row.try_get(13)?,
                        input_schema_json: row.try_get(14)?,
                        output_schema_json: row.try_get(15)?,
                        read_only: row.try_get(16)?,
                        destructive: row.try_get(17)?,
                        idempotent: row.try_get(18)?,
                        open_world: row.try_get(19)?,
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
