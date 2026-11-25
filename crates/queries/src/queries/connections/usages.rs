// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub connection_id: uuid::Uuid,
    pub kind: String,
    pub id: Option<uuid::Uuid>,
    pub name: String,
    pub detail: String,
    pub resource_kind: Option<String>,
    pub resource_id: Option<uuid::Uuid>,
}
pub struct RecordBorrowed<'a> {
    pub connection_id: uuid::Uuid,
    pub kind: &'a str,
    pub id: Option<uuid::Uuid>,
    pub name: &'a str,
    pub detail: &'a str,
    pub resource_kind: Option<&'a str>,
    pub resource_id: Option<uuid::Uuid>,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            connection_id,
            kind,
            id,
            name,
            detail,
            resource_kind,
            resource_id,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            connection_id,
            kind: kind.into(),
            id,
            name: name.into(),
            detail: detail.into(),
            resource_kind: resource_kind.map(|v| v.into()),
            resource_id,
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
        "SELECT u.connection_id,u.kind,u.id,u.name,u.detail,u.resource_kind,u.resource_id FROM ( SELECT ca.connection_id,'agent' AS kind,a.id,a.name,ca.capability AS detail,'agent' AS resource_kind,a.id AS resource_id FROM connection_agents ca JOIN agents a ON a.id=ca.agent_id WHERE ca.connection_id=ANY($1::UUID[]) UNION ALL SELECT p.connection_id,'personal_agent',a.id,a.name,'','agent',a.id FROM personal_connection_agents p JOIN agents a ON a.id=p.agent_id WHERE p.connection_id=ANY($1::UUID[]) UNION ALL SELECT s.connection_id,'tool_source',a.id,a.name,s.slug,'agent',a.id FROM agent_tool_sources s JOIN agents a ON a.id=s.agent_id WHERE s.connection_id=ANY($1::UUID[]) UNION ALL SELECT s.connection_id,'sandbox_tool_source',b.id,b.name,s.slug,'sandbox_blueprint',b.id FROM agent_tool_sources s JOIN sandbox_blueprints b ON b.id=s.sandbox_blueprint_id WHERE s.connection_id=ANY($1::UUID[]) UNION ALL SELECT r.connection_id,'routine',r.id,r.name,a.name||', '||r.signal_type,'agent',a.id FROM routines r JOIN agents a ON a.id=r.agent_id WHERE r.connection_id=ANY($1::UUID[]) UNION ALL SELECT b.connection_id,'sandbox_blueprint',b.id,b.name,'','sandbox_blueprint',b.id FROM sandbox_blueprints b WHERE b.connection_id=ANY($1::UUID[]) UNION ALL SELECT s.connection_id,'sandbox',s.id,b.name,s.status,'sandbox_blueprint',b.id FROM sandboxes s JOIN sandbox_blueprints b ON b.id=s.blueprint_id WHERE s.connection_id=ANY($1::UUID[]) AND b.connection_id<>s.connection_id UNION ALL SELECT g.connection_id,'sandbox_egress',b.id,b.name,g.slug,'sandbox_blueprint',b.id FROM sandbox_blueprint_egress g JOIN sandbox_blueprints b ON b.id=g.blueprint_id WHERE g.connection_id=ANY($1::UUID[]) UNION ALL SELECT m.connection_id,'memory',NULL,'Memory embeddings',m.model,NULL,NULL FROM memory_settings m WHERE m.connection_id=ANY($1::UUID[]) UNION ALL SELECT s.connection_id,'memory_synthesis',k.id,k.name,s.model,'memory_bank',k.id FROM memory_synthesis s JOIN memory_banks k ON k.id=s.bank_id WHERE s.connection_id=ANY($1::UUID[]) ) u ORDER BY u.connection_id,u.kind,u.name,u.detail",
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
    pub fn bind<'c, 'a, 's, C: GenericClient, T1: crate::ArraySql<Item = uuid::Uuid>>(
        &'s self,
        client: &'c C,
        ids: &'a T1,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 1> {
        RecordQuery {
            client,
            params: [ids],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        connection_id: row.try_get(0)?,
                        kind: row.try_get(1)?,
                        id: row.try_get(2)?,
                        name: row.try_get(3)?,
                        detail: row.try_get(4)?,
                        resource_kind: row.try_get(5)?,
                        resource_id: row.try_get(6)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
