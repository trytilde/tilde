// This file was generated with `cornucopia`. Do not modify.

#[derive(Debug)]
pub struct RunParams<T1: crate::StringSql, T2: crate::ArraySql<Item = uuid::Uuid>> {
    pub p1: uuid::Uuid,
    pub p2: uuid::Uuid,
    pub p3: uuid::Uuid,
    pub p4: T1,
    pub p5: Option<uuid::Uuid>,
    pub p6: T2,
    pub p7: i64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: uuid::Uuid,
    pub title: String,
    pub status: String,
    pub goal_id: Option<uuid::Uuid>,
    pub blocked_reason: String,
    pub dependencies: Vec<uuid::Uuid>,
    pub transaction_id: i64,
}
pub struct RecordBorrowed<'a> {
    pub id: uuid::Uuid,
    pub title: &'a str,
    pub status: &'a str,
    pub goal_id: Option<uuid::Uuid>,
    pub blocked_reason: &'a str,
    pub dependencies: crate::ArrayIterator<'a, uuid::Uuid>,
    pub transaction_id: i64,
}
impl<'a> From<RecordBorrowed<'a>> for Record {
    fn from(
        RecordBorrowed {
            id,
            title,
            status,
            goal_id,
            blocked_reason,
            dependencies,
            transaction_id,
        }: RecordBorrowed<'a>,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            status: status.into(),
            goal_id,
            blocked_reason: blocked_reason.into(),
            dependencies: dependencies.map(|v| v).collect(),
            transaction_id,
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
        "WITH created AS ( INSERT INTO chat_tasks(id, thread_id, agent_id, title, goal_id) VALUES($1, $2, $3, $4, $5) ON CONFLICT(id) DO NOTHING RETURNING id, title, status, goal_id, blocked_reason ), linked AS ( INSERT INTO chat_task_dependencies(thread_id, agent_id, task_id, dependency_id) SELECT $2, $3, created.id, dep FROM created CROSS JOIN UNNEST($6::UUID[]) AS dep RETURNING dependency_id ), selected AS MATERIALIZED ( SELECT c.id, c.title, c.status, c.goal_id, c.blocked_reason, ARRAY(SELECT dependency_id FROM linked ORDER BY dependency_id) AS dependencies FROM created c UNION ALL SELECT t.id, t.title, t.status, t.goal_id, t.blocked_reason, ARRAY(SELECT dependency_id FROM chat_task_dependencies WHERE task_id = t.id ORDER BY dependency_id) FROM chat_tasks t WHERE t.id = $1 AND t.thread_id = $2 AND t.agent_id = $3 AND NOT EXISTS(SELECT 1 FROM created) ), valid AS MATERIALIZED ( SELECT * FROM selected WHERE title = $4 AND goal_id IS NOT DISTINCT FROM $5 AND dependencies = $6 ), advanced AS ( UPDATE chat_threads SET activity_sequence = $7::BIGINT WHERE id = $2 AND activity_sequence = $7::BIGINT - 1 AND EXISTS(SELECT 1 FROM valid) RETURNING id ) SELECT v.id AS id, v.title AS title, v.status AS status, v.goal_id, v.blocked_reason AS blocked_reason, v.dependencies AS dependencies, txid_current() AS transaction_id FROM valid v WHERE EXISTS(SELECT 1 FROM advanced)",
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
    pub fn bind<
        'c,
        'a,
        's,
        C: GenericClient,
        T1: crate::StringSql,
        T2: crate::ArraySql<Item = uuid::Uuid>,
    >(
        &'s self,
        client: &'c C,
        p1: &'a uuid::Uuid,
        p2: &'a uuid::Uuid,
        p3: &'a uuid::Uuid,
        p4: &'a T1,
        p5: &'a Option<uuid::Uuid>,
        p6: &'a T2,
        p7: &'a i64,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 7> {
        RecordQuery {
            client,
            params: [p1, p2, p3, p4, p5, p6, p7],
            query: self.0,
            cached: self.1.as_ref(),
            extractor:
                |row: &tokio_postgres::Row| -> Result<RecordBorrowed, tokio_postgres::Error> {
                    Ok(RecordBorrowed {
                        id: row.try_get(0)?,
                        title: row.try_get(1)?,
                        status: row.try_get(2)?,
                        goal_id: row.try_get(3)?,
                        blocked_reason: row.try_get(4)?,
                        dependencies: row.try_get(5)?,
                        transaction_id: row.try_get(6)?,
                    })
                },
            mapper: |it| Record::from(it),
        }
    }
}
impl<'c, 'a, 's, C: GenericClient, T1: crate::StringSql, T2: crate::ArraySql<Item = uuid::Uuid>>
    crate::client::async_::Params<
        'c,
        'a,
        's,
        RunParams<T1, T2>,
        RecordQuery<'c, 'a, 's, C, Record, 7>,
        C,
    > for RunStmt
{
    fn params(
        &'s self,
        client: &'c C,
        params: &'a RunParams<T1, T2>,
    ) -> RecordQuery<'c, 'a, 's, C, Record, 7> {
        self.bind(
            client, &params.p1, &params.p2, &params.p3, &params.p4, &params.p5, &params.p6,
            &params.p7,
        )
    }
}
