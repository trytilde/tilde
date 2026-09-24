// This file was generated with `cornucopia`. Do not modify.

#[derive(Clone, Copy, Debug)]
pub struct TargetsParams {
    pub message: uuid::Uuid,
    pub thread: uuid::Uuid,
    pub participant: uuid::Uuid,
}
#[derive(Clone, Copy, Debug)]
pub struct AttachmentParams {
    pub thread: uuid::Uuid,
    pub message: uuid::Uuid,
    pub attachment: uuid::Uuid,
}
use crate::client::async_::GenericClient;
use futures::{self, StreamExt, TryStreamExt};
pub struct TargetsStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn targets() -> TargetsStmt {
    TargetsStmt(
        "INSERT INTO chat_message_targets(message_id,participant_id) SELECT $1,id FROM chat_participants WHERE thread_id=$2 AND id=$3 ON CONFLICT(message_id,participant_id) DO NOTHING",
        None,
    )
}
impl TargetsStmt {
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
        message: &'a uuid::Uuid,
        thread: &'a uuid::Uuid,
        participant: &'a uuid::Uuid,
    ) -> Result<u64, tokio_postgres::Error> {
        client
            .execute(self.0, &[message, thread, participant])
            .await
    }
}
impl<'a, C: GenericClient + Send + Sync>
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        TargetsParams,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for TargetsStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a TargetsParams,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(client, &params.message, &params.thread, &params.participant))
    }
}
pub struct AttachmentStmt(&'static str, Option<tokio_postgres::Statement>);
pub fn attachment() -> AttachmentStmt {
    AttachmentStmt(
        "INSERT INTO chat_message_attachments(thread_id,message_id,attachment_id) VALUES($1,$2,$3) ON CONFLICT(message_id,attachment_id) DO NOTHING",
        None,
    )
}
impl AttachmentStmt {
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
        thread: &'a uuid::Uuid,
        message: &'a uuid::Uuid,
        attachment: &'a uuid::Uuid,
    ) -> Result<u64, tokio_postgres::Error> {
        client.execute(self.0, &[thread, message, attachment]).await
    }
}
impl<'a, C: GenericClient + Send + Sync>
    crate::client::async_::Params<
        'a,
        'a,
        'a,
        AttachmentParams,
        std::pin::Pin<
            Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
        >,
        C,
    > for AttachmentStmt
{
    fn params(
        &'a self,
        client: &'a C,
        params: &'a AttachmentParams,
    ) -> std::pin::Pin<
        Box<dyn futures::Future<Output = Result<u64, tokio_postgres::Error>> + Send + 'a>,
    > {
        Box::pin(self.bind(client, &params.thread, &params.message, &params.attachment))
    }
}
