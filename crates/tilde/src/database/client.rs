//! A pooled connection that holds no session state.
//!
//! `DATABASE_URL` may point at a transaction-pooling PgBouncer, where consecutive transactions
//! of one client connection can run on different server sessions: a session `SET` would apply to
//! whichever session ran it and leak into someone else's work. So per-checkout state is
//! transaction-scoped instead. When the checkout carries a `scope` (statements that set it with
//! `SET LOCAL` and `set_config(..., true)`), every statement run directly on the client runs in
//! its own transaction that starts with it, and [`Client::transaction`] starts with it too.
//! Without one, statements run as plain autocommit statements of the login role.
use super::Transaction;
use futures::FutureExt;
use tilde_queries::client::GenericClient;
use tokio_postgres::{
    Error, Row, RowStream, Statement, ToStatement,
    types::{BorrowToSql, ToSql},
};

pub struct Client {
    inner: deadpool_postgres::Client,
    scope: Option<String>,
    /// Statement transactions take turns: two in flight on one connection would share, and
    /// commit, each other's transaction.
    turn: tokio::sync::Mutex<()>,
}

impl Client {
    pub(super) fn new(inner: deadpool_postgres::Client, scope: Option<String>) -> Self {
        Self {
            inner,
            scope,
            turn: tokio::sync::Mutex::new(()),
        }
    }

    /// Begin a transaction carrying the checkout's scope.
    pub async fn transaction(&mut self) -> Result<Transaction<'_>, Error> {
        let tx = self.inner.transaction().await?;
        if let Some(scope) = &self.scope {
            tx.batch_execute(scope).await?;
        }
        Ok(tx)
    }

    /// Simple-protocol statements, in the checkout's scope.
    pub async fn batch_execute(&self, sql: &str) -> Result<(), Error> {
        self.scoped(async || self.inner.batch_execute(sql).await)
            .await
    }

    pub async fn execute<T>(
        &self,
        statement: &T,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<u64, Error>
    where
        T: ?Sized + ToStatement + Sync + Send,
    {
        GenericClient::execute(self, statement, params).await
    }
    pub async fn query<T>(
        &self,
        statement: &T,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Vec<Row>, Error>
    where
        T: ?Sized + ToStatement + Sync + Send,
    {
        GenericClient::query(self, statement, params).await
    }
    pub async fn query_one<T>(
        &self,
        statement: &T,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Row, Error>
    where
        T: ?Sized + ToStatement + Sync + Send,
    {
        GenericClient::query_one(self, statement, params).await
    }
    pub async fn query_opt<T>(
        &self,
        statement: &T,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Option<Row>, Error>
    where
        T: ?Sized + ToStatement + Sync + Send,
    {
        GenericClient::query_opt(self, statement, params).await
    }

    /// Run `statement` in its own transaction that begins with the scope. BEGIN and the
    /// statement are pipelined (the statement is sent behind BEGIN on the same connection, so
    /// it can only run inside it; if the scope fails, the aborted transaction fails it too).
    async fn scoped<T>(
        &self,
        statement: impl AsyncFnOnce() -> Result<T, Error>,
    ) -> Result<T, Error> {
        let Some(scope) = &self.scope else {
            return statement().await;
        };
        let _turn = self.turn.lock().await;
        let open = Open(Some(&self.inner));
        let begin = format!("BEGIN; {scope}");
        let (began, result) = futures::join!(self.inner.batch_execute(&begin), statement());
        let end = if began.is_ok() && result.is_ok() {
            "COMMIT"
        } else {
            "ROLLBACK"
        };
        let ended = self.inner.batch_execute(end).await;
        open.close();
        began?;
        let value = result?;
        ended?;
        Ok(value)
    }
}

/// An open statement transaction; a future cancelled before its end rolls it back, so the
/// connection never returns to the pool inside it.
struct Open<'a>(Option<&'a tokio_postgres::Client>);
impl Open<'_> {
    fn close(mut self) {
        self.0 = None;
    }
}
impl Drop for Open<'_> {
    fn drop(&mut self) {
        if let Some(client) = self.0 {
            send(client, "ROLLBACK");
        }
    }
}

/// Queue `sql` on the connection without waiting for its result. Requests on one connection
/// run in order, so whatever is sent afterwards runs after it (what `tokio_postgres`'s own
/// `Transaction` drop does to roll back).
fn send(client: &tokio_postgres::Client, sql: &str) {
    // The first poll encodes and queues the request; the reply is discarded.
    let _ = client.batch_execute(sql).now_or_never();
}

impl GenericClient for Client {
    fn stmt_cache() -> bool {
        true
    }
    /// Names resolve when the statement is parsed, so it is prepared in the checkout's scope
    /// too: behind a transaction pooler the search path exists only inside it.
    async fn prepare(&self, query: &str) -> Result<Statement, Error> {
        self.scoped(async || self.inner.prepare_cached(query).await)
            .await
    }
    async fn execute<T>(&self, query: &T, params: &[&(dyn ToSql + Sync)]) -> Result<u64, Error>
    where
        T: ?Sized + ToStatement + Sync + Send,
    {
        self.scoped(async || self.inner.execute(query, params).await)
            .await
    }
    async fn query_one<T>(
        &self,
        statement: &T,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Row, Error>
    where
        T: ?Sized + ToStatement + Sync + Send,
    {
        self.scoped(async || self.inner.query_one(statement, params).await)
            .await
    }
    async fn query_opt<T>(
        &self,
        statement: &T,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Option<Row>, Error>
    where
        T: ?Sized + ToStatement + Sync + Send,
    {
        self.scoped(async || self.inner.query_opt(statement, params).await)
            .await
    }
    async fn query<T>(&self, query: &T, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>, Error>
    where
        T: ?Sized + ToStatement + Sync + Send,
    {
        self.scoped(async || self.inner.query(query, params).await)
            .await
    }
    /// The rows stream after this returns, so the statement's transaction cannot be ended
    /// after them here: COMMIT is queued behind the statement without waiting for its reply.
    /// It runs before anything else on the connection, but the caller may see the last row
    /// before it is durable. Writes whose results stream (`.all()` of `INSERT ... RETURNING`)
    /// and must be visible to the next unit of work at once belong in an explicit transaction.
    async fn query_raw<T, I>(&self, statement: &T, params: I) -> Result<RowStream, Error>
    where
        T: ?Sized + ToStatement + Sync + Send,
        I: IntoIterator + Sync + Send,
        I::IntoIter: ExactSizeIterator,
        I::Item: BorrowToSql,
    {
        let Some(scope) = &self.scope else {
            return self.inner.query_raw(statement, params).await;
        };
        let _turn = self.turn.lock().await;
        let open = Open(Some(&self.inner));
        let begin = format!("BEGIN; {scope}");
        let (began, rows) = futures::join!(
            self.inner.batch_execute(&begin),
            self.inner.query_raw(statement, params)
        );
        let ok = began.is_ok() && rows.is_ok();
        open.close();
        send(&self.inner, if ok { "COMMIT" } else { "ROLLBACK" });
        began?;
        rows
    }
}
