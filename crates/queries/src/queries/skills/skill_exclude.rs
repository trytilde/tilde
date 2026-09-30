// This file was generated with `cornucopia`. Do not modify.

#[derive(Clone, Copy, Debug)]
pub struct RunParams {
    pub agent: uuid::Uuid,
    pub skill_id: uuid::Uuid,
    pub excluded: bool,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct RunStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn run() -> RunStmt {
    RunStmt(
        "WITH restored AS ( DELETE FROM agent_skill_exclusions WHERE agent_id=$1 AND skill_id=$2 AND NOT $3 ), single AS ( DELETE FROM agent_skills WHERE agent_id=$1 AND skill_id=$2 AND $3 ) INSERT INTO agent_skill_exclusions(agent_id,skill_id) SELECT $1, $2 WHERE $3 ON CONFLICT DO NOTHING",
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
        agent: &'a uuid::Uuid,
        skill_id: &'a uuid::Uuid,
        excluded: &'a bool,
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[agent, skill_id, excluded]).await
    }
}
impl<'a, C: GenericClient + Send + Sync>
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        RunParams,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for RunStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a RunParams,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(client, &params.agent, &params.skill_id, &params.excluded))
    }
}
