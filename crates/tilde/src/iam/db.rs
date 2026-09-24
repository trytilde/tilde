//! Typed PostgreSQL operations generated from the SQL contracts. Callers own transactions.
#![allow(clippy::too_many_arguments)]
use crate::database::{DbError, DbResult, GenericClient};

fn agent_lock_row(
    r: tilde_queries::queries::iam::agent_lock::Record,
) -> DbResult<crate::agent::Agent> {
    Ok(crate::agent::Agent {
        id: r.id,
        name: r.name,
        concurrency_policy: serde_json::from_value(serde_json::Value::String(r.concurrency_policy))
            .map_err(crate::database::DbError::decode)?,
        avatar_seed: r.avatar_seed,
        avatar_key: r.avatar_key,
        paused: r.paused,

        created_at: r.created_at,
        updated_at: r.updated_at,
        capabilities: tokio_postgres::types::Json(
            serde_json::from_value(r.capabilities).map_err(crate::database::DbError::decode)?,
        ),
    })
}

pub async fn agent_lock_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
) -> DbResult<Option<crate::agent::Agent>> {
    tilde_queries::queries::iam::agent_lock::run()
        .bind(db, &p1)
        .opt()
        .await?
        .map(agent_lock_row)
        .transpose()
}

pub use tilde_queries::queries::iam::inference_connections::Record as InferenceConnectionRow;

pub async fn inference_connections_all(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
) -> DbResult<Vec<InferenceConnectionRow>> {
    Ok(tilde_queries::queries::iam::inference_connections::run()
        .bind(db, &p1, &p2)
        .all()
        .await?)
}

pub async fn cleanup_execute(db: &impl GenericClient) -> DbResult<u64> {
    Ok(tilde_queries::queries::iam::cleanup::run().bind(db).await?)
}

pub struct InvocationLiveRow {
    pub participant_id: uuid::Uuid,
    pub capabilities: tokio_postgres::types::Json<crate::iam::capabilities::Capabilities>,
}

fn invocation_live_row(
    r: tilde_queries::queries::iam::invocation_live::Record,
) -> DbResult<InvocationLiveRow> {
    Ok(InvocationLiveRow {
        participant_id: r.participant_id,
        capabilities: tokio_postgres::types::Json(
            serde_json::from_value(r.capabilities).map_err(crate::database::DbError::decode)?,
        ),
    })
}

pub async fn invocation_live_opt(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: uuid::Uuid,
) -> DbResult<Option<InvocationLiveRow>> {
    tilde_queries::queries::iam::invocation_live::run()
        .bind(db, &p1, &p2, &p3, &p4)
        .opt()
        .await?
        .map(invocation_live_row)
        .transpose()
}

pub use tilde_queries::queries::iam::invocation_trace_live::Record as InvocationTraceLiveRow;

pub async fn invocation_trace_live_one(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: uuid::Uuid,
    p3: uuid::Uuid,
    p4: uuid::Uuid,
    p5: f64,
) -> DbResult<InvocationTraceLiveRow> {
    tilde_queries::queries::iam::invocation_trace_live::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub use tilde_queries::queries::iam::key_get::Record as KeyGetRow;

pub async fn key_get_one(db: &impl GenericClient) -> DbResult<KeyGetRow> {
    tilde_queries::queries::iam::key_get::run()
        .bind(db)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

pub async fn key_insert_execute(db: &impl GenericClient, p1: &[u8]) -> DbResult<u64> {
    Ok(tilde_queries::queries::iam::key_insert::run()
        .bind(db, &p1)
        .await?)
}

pub async fn session_create_execute(
    db: &impl GenericClient,
    p1: &[u8],
    p2: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::iam::session_create::run()
        .bind(db, &p1, &p2)
        .await?)
}

pub async fn session_delete_execute(db: &impl GenericClient, p1: &[u8]) -> DbResult<u64> {
    Ok(tilde_queries::queries::iam::session_delete::run()
        .bind(db, &p1)
        .await?)
}

pub use tilde_queries::queries::iam::session_get::Record as SessionGetRow;

pub async fn session_get_opt(
    db: &impl GenericClient,
    p1: &[u8],
) -> DbResult<Option<SessionGetRow>> {
    Ok(tilde_queries::queries::iam::session_get::run()
        .bind(db, &p1)
        .opt()
        .await?)
}

pub use tilde_queries::queries::iam::state_consume::Record as StateConsumeRow;

pub async fn state_consume_opt(
    db: &impl GenericClient,
    p1: &[u8],
    p2: &[u8],
) -> DbResult<Option<StateConsumeRow>> {
    Ok(tilde_queries::queries::iam::state_consume::run()
        .bind(db, &p1, &p2)
        .opt()
        .await?)
}

pub async fn state_create_execute(
    db: &impl GenericClient,
    p1: uuid::Uuid,
    p2: &[u8],
    p3: &str,
    p4: &[u8],
    p5: &[u8],
) -> DbResult<u64> {
    Ok(tilde_queries::queries::iam::state_create::run()
        .bind(db, &p1, &p2, &p3, &p4, &p5)
        .await?)
}

pub use tilde_queries::queries::iam::user_upsert::Record as UserUpsertRow;

pub async fn user_upsert_one(
    db: &impl GenericClient,
    id: uuid::Uuid,
    issuer: &str,
    subject: &str,
    email: Option<&str>,
    display_name: Option<&str>,
) -> DbResult<UserUpsertRow> {
    tilde_queries::queries::iam::user_upsert::run()
        .bind(db, &id, &issuer, &subject, &email, &display_name)
        .opt()
        .await?
        .ok_or(DbError::NotFound)
}

/// Reconcile provider-owned memberships with the groups asserted at this login.
pub async fn external_groups_sync(
    db: &impl GenericClient,
    user: uuid::Uuid,
    ids: &[String],
    names: &[String],
) -> DbResult<()> {
    use tilde_queries::queries::iam as q;
    q::external_groups_upsert::run()
        .bind(db, &ids, &names)
        .await?;
    q::external_groups_prune::run()
        .bind(db, &user, &ids)
        .await?;
    q::external_groups_join::run().bind(db, &user, &ids).await?;
    Ok(())
}

pub async fn sessions_revoke(db: &impl GenericClient, user: uuid::Uuid) -> DbResult<u64> {
    Ok(tilde_queries::queries::iam::sessions_revoke::run()
        .bind(db, &user)
        .await?)
}

// Management API key digests stay inside the IAM persistence boundary.

use crate::proto::tilde::management::v1 as wire;

fn api_key_wire(
    id: uuid::Uuid,
    name: String,
    prefix: String,
    created_at: chrono::DateTime<chrono::Utc>,
    revoked_at: Option<chrono::DateTime<chrono::Utc>>,
    user: Option<uuid::Uuid>,
    roles: Vec<types::Role>,
) -> wire::ApiKey {
    wire::ApiKey {
        id: id.to_string(),
        name,
        prefix,
        created_at: crate::chat::audit::timestamp(created_at).into(),
        revoked_at: revoked_at.map(crate::chat::audit::timestamp).into(),
        created_by_user_id: user.map(|id| id.to_string()),
        roles,
        ..Default::default()
    }
}
pub async fn api_key_create(
    db: &impl GenericClient,
    id: uuid::Uuid,
    name: &str,
    prefix: &str,
    hash: &[u8],
    user: uuid::Uuid,
) -> DbResult<wire::ApiKey> {
    let r = tilde_queries::queries::iam::api_key_create::run()
        .bind(db, &id, &name, &prefix, &hash, &user)
        .one()
        .await?;
    Ok(api_key_wire(
        r.id,
        r.name,
        r.prefix,
        r.created_at,
        r.revoked_at,
        r.created_by_user_id,
        Vec::new(),
    ))
}
pub async fn api_key_list(
    db: &impl GenericClient,
    after: Option<uuid::Uuid>,
    limit: i64,
    caller: &Access,
) -> DbResult<Vec<wire::ApiKey>> {
    Ok(tilde_queries::queries::iam::api_key_list::run()
        .bind(db, &after, &caller.admin, &caller.user, &limit)
        .all()
        .await?
        .into_iter()
        .map(|r| {
            let roles = r
                .role_ids
                .into_iter()
                .zip(r.role_names)
                .zip(r.role_descriptions)
                .map(|((id, name), description)| types::Role {
                    id,
                    name,
                    description,
                    ..Default::default()
                })
                .collect();
            api_key_wire(
                r.id,
                r.name,
                r.prefix,
                r.created_at,
                r.revoked_at,
                r.created_by_user_id,
                roles,
            )
        })
        .collect())
}
pub async fn api_key_authenticate(
    db: &impl GenericClient,
    hash: &[u8],
) -> DbResult<Option<uuid::Uuid>> {
    Ok(tilde_queries::queries::iam::api_key_authenticate::run()
        .bind(db, &hash)
        .opt()
        .await?
        .map(|r| r.id))
}
pub async fn api_key_revoke(
    db: &impl GenericClient,
    id: uuid::Uuid,
    caller: &Access,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::iam::api_key_revoke::run()
        .bind(db, &id, &caller.admin, &caller.user)
        .await?)
}

// Roles, memberships and groups.

use super::authz::{Access, Action, Grantee, Kind, Resource};
use crate::proto::tilde::types::v1 as types;

pub async fn held(
    db: &impl GenericClient,
    kind: Kind,
    action: Action,
    ids: &[uuid::Uuid],
    access: &Access,
) -> DbResult<Vec<(uuid::Uuid, super::authz::Held)>> {
    Ok(tilde_queries::queries::iam::held::run()
        .bind(
            db,
            &kind.as_str(),
            &action.name(),
            &access.admin,
            &access.user,
            &access.api_key,
            &access.groups,
            &ids,
        )
        .all()
        .await?
        .into_iter()
        .map(|r| {
            (
                r.id,
                super::authz::Held {
                    held: r.held,
                    visible: r.visible,
                },
            )
        })
        .collect())
}
pub async fn actions_held(
    db: &impl GenericClient,
    resource: Resource,
    access: &Access,
) -> DbResult<Vec<Action>> {
    let names: Vec<&str> = resource.kind.actions().iter().map(|a| a.name()).collect();
    Ok(tilde_queries::queries::iam::access::run()
        .bind(
            db,
            &names,
            &resource.kind.as_str(),
            &resource.id,
            &access.admin,
            &access.user,
            &access.api_key,
            &access.groups,
        )
        .all()
        .await?
        .iter()
        .filter_map(|name| resource.kind.action(name))
        .collect())
}
pub async fn role_create(
    db: &impl GenericClient,
    id: &str,
    name: &str,
    description: &str,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::iam::role_create::run()
        .bind(db, &id, &name, &description)
        .await?)
}
pub async fn role_statement_put(
    db: &impl GenericClient,
    role: &str,
    resource: Resource,
    action: Action,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::iam::role_statement_put::run()
        .bind(
            db,
            &role,
            &resource.kind.as_str(),
            &resource.id,
            &action.name(),
        )
        .await?)
}
pub async fn role_get(db: &impl GenericClient, id: &str) -> DbResult<Option<types::Role>> {
    Ok(tilde_queries::queries::iam::role_get::run()
        .bind(db, &id)
        .opt()
        .await?
        .map(|r| types::Role {
            id: r.id,
            name: r.name,
            description: r.description,
            ..Default::default()
        }))
}
/// Statements with an unknown kind or action are skipped: the vocabulary shrank under them.
pub async fn role_statements(
    db: &impl GenericClient,
    role: &str,
) -> DbResult<Vec<(Resource, Action)>> {
    Ok(tilde_queries::queries::iam::role_statements::run()
        .bind(db, &role)
        .all()
        .await?
        .into_iter()
        .filter_map(|r| {
            let kind = Kind::parse(&r.resource_kind)?;
            Some((
                Resource {
                    kind,
                    id: r.resource_id,
                },
                kind.action(&r.action)?,
            ))
        })
        .collect())
}
pub async fn role_list(db: &impl GenericClient, resource: Resource) -> DbResult<Vec<types::Role>> {
    Ok(tilde_queries::queries::iam::role_list::run()
        .bind(db, &resource.kind.as_str(), &resource.id)
        .all()
        .await?
        .into_iter()
        .map(|r| types::Role {
            id: r.id,
            name: r.name,
            description: r.description,
            ..Default::default()
        })
        .collect())
}
/// False when the principal does not exist.
pub async fn role_member_put(
    db: &impl GenericClient,
    role: &str,
    principal: &Grantee,
    by: &Access,
) -> DbResult<bool> {
    let (group, user, api_key) = principal.columns();
    Ok(tilde_queries::queries::iam::role_member_put::run()
        .bind(db, &role, &group, &user, &api_key, &by.user)
        .opt()
        .await?
        .is_some())
}
pub async fn role_member_delete(
    db: &impl GenericClient,
    role: &str,
    principal: &Grantee,
) -> DbResult<u64> {
    let (group, user, api_key) = principal.columns();
    Ok(tilde_queries::queries::iam::role_member_delete::run()
        .bind(db, &role, &group, &user, &api_key)
        .await?)
}
pub struct RoleAssignmentRow {
    pub grantee: Grantee,
    pub role: types::Role,
    pub label: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
pub async fn role_assignments(
    db: &impl GenericClient,
    resource: Resource,
) -> DbResult<Vec<RoleAssignmentRow>> {
    Ok(tilde_queries::queries::iam::role_assignments::run()
        .bind(db, &resource.kind.as_str(), &resource.id)
        .all()
        .await?
        .into_iter()
        .map(|r| RoleAssignmentRow {
            grantee: match (r.group_id, r.user_id, r.api_key_id) {
                (Some(id), _, _) => Grantee::Group(id),
                (_, Some(id), _) => Grantee::User(id),
                (_, _, id) => Grantee::ApiKey(id.unwrap_or_default()),
            },
            role: types::Role {
                id: r.role_id,
                name: r.name,
                description: r.description,
                ..Default::default()
            },
            label: r.label.unwrap_or_default(),
            created_at: r.created_at,
        })
        .collect())
}
pub async fn roles_purge(db: &impl GenericClient, resource: Resource) -> DbResult<u64> {
    Ok(tilde_queries::queries::iam::roles_purge::run()
        .bind(db, &resource.kind.as_str(), &resource.id)
        .await?)
}

pub use tilde_queries::queries::iam::group_get::Record as GroupRow;
pub async fn group_create(
    db: &impl GenericClient,
    id: &str,
    name: &str,
    source: &str,
) -> DbResult<Option<GroupRow>> {
    Ok(tilde_queries::queries::iam::group_create::run()
        .bind(db, &id, &name, &source)
        .opt()
        .await?
        .map(|r| GroupRow {
            id: r.id,
            name: r.name,
            source: r.source,
            created_at: r.created_at,
            member_count: r.member_count,
        }))
}
pub async fn group_get(db: &impl GenericClient, id: &str) -> DbResult<Option<GroupRow>> {
    Ok(tilde_queries::queries::iam::group_get::run()
        .bind(db, &id)
        .opt()
        .await?)
}
pub async fn group_list(
    db: &impl GenericClient,
    after: Option<&str>,
    limit: i64,
    search: &str,
) -> DbResult<Vec<GroupRow>> {
    Ok(tilde_queries::queries::iam::group_list::run()
        .bind(db, &after, &search, &limit)
        .all()
        .await?
        .into_iter()
        .map(|r| GroupRow {
            id: r.id,
            name: r.name,
            source: r.source,
            created_at: r.created_at,
            member_count: r.member_count,
        })
        .collect())
}
pub async fn group_delete(db: &impl GenericClient, id: &str) -> DbResult<u64> {
    Ok(tilde_queries::queries::iam::group_delete::run()
        .bind(db, &id)
        .await?)
}
/// False when the group or user does not exist, or the membership already did.
pub async fn group_member_add(
    db: &impl GenericClient,
    group: &str,
    user: uuid::Uuid,
) -> DbResult<bool> {
    Ok(tilde_queries::queries::iam::group_member_add::run()
        .bind(db, &group, &user)
        .opt()
        .await?
        .is_some())
}
pub async fn group_member_remove(
    db: &impl GenericClient,
    group: &str,
    user: uuid::Uuid,
) -> DbResult<u64> {
    Ok(tilde_queries::queries::iam::group_member_remove::run()
        .bind(db, &group, &user)
        .await?)
}
pub use tilde_queries::queries::iam::group_member_list::Record as GroupMemberRow;
pub async fn group_member_list(
    db: &impl GenericClient,
    group: &str,
    after: Option<uuid::Uuid>,
    limit: i64,
) -> DbResult<Vec<GroupMemberRow>> {
    Ok(tilde_queries::queries::iam::group_member_list::run()
        .bind(db, &group, &after, &limit)
        .all()
        .await?)
}
pub use tilde_queries::queries::iam::user_get::Record as UserRow;
pub async fn user_get(db: &impl GenericClient, id: uuid::Uuid) -> DbResult<Option<UserRow>> {
    Ok(tilde_queries::queries::iam::user_get::run()
        .bind(db, &id)
        .opt()
        .await?)
}
pub async fn user_list(
    db: &impl GenericClient,
    after: Option<uuid::Uuid>,
    limit: i64,
    search: &str,
) -> DbResult<Vec<UserRow>> {
    Ok(tilde_queries::queries::iam::user_list::run()
        .bind(db, &after, &search, &limit)
        .all()
        .await?
        .into_iter()
        .map(|r| UserRow {
            id: r.id,
            issuer: r.issuer,
            subject: r.subject,
            email: r.email,
            display_name: r.display_name,
            group_ids: r.group_ids,
        })
        .collect())
}
