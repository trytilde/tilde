// This file was generated with `cornucopia`. Do not modify.

use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "DELETE FROM telemetry_objects WHERE created_at<NOW()-make_interval(secs=>$1::INTEGER) AND leased_until<=NOW()",
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
        retention_seconds: &'a i32,
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[retention_seconds]).await
    }
}
