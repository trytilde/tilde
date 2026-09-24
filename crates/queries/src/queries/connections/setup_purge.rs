// This file was generated with `cornucopia`. Do not modify.

use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "WITH expired AS ( DELETE FROM connection_setups WHERE created_at<=now()-interval '10 minutes' OR expires_at<=now() RETURNING connection_id ) UPDATE connections SET status='failed',updated_at=now() WHERE id IN (SELECT connection_id FROM expired) AND status NOT IN ('ready','disconnected')",
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
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[]).await
    }
}
