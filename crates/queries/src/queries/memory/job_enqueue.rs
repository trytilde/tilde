// This file was generated with `cornucopia`. Do not modify.

use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "INSERT INTO memory_embedding_jobs(memory_id) SELECT m.id FROM memories m WHERE ($1::UUID IS NULL OR m.id=$1) AND m.deleted_at IS NULL AND NOT EXISTS (SELECT 1 FROM memories s WHERE s.supersedes_id=m.id) ON CONFLICT(memory_id) DO UPDATE SET requested_at=clock_timestamp(),run_after=NOW(),attempts=0",
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
        p1: &'a Option<uuid::Uuid>,
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[p1]).await
    }
}
