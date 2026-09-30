// This file was generated with `cornucopia`. Do not modify.

use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "WITH provider AS (SELECT provider_id FROM connection_providers WHERE tool_host_id=$1), removed AS (DELETE FROM connections WHERE provider_id IN (SELECT provider_id FROM provider)), types AS (DELETE FROM connection_types WHERE provider_id IN (SELECT provider_id FROM provider)), parameters AS (DELETE FROM connection_oauth_parameters WHERE provider_id IN (SELECT provider_id FROM provider)) DELETE FROM connection_oauth_result_fields WHERE provider_id IN (SELECT provider_id FROM provider)",
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
    pub async fn bind<'c, 'a, 's, C: GenericClient>(
        &'s self,
        client: &'c C,
        p1: &'a uuid::Uuid,
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[p1]).await
    }
}
