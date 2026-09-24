//! Management of groups, memberships and role assignments. Group and user administration is
//! for administrators; assigning a role is open to anyone within reach of it.
use super::authz::{self, Grantee, Kind, Resource};
use super::db;
use crate::database::Pool;
use crate::proto::tilde::{management::v1 as wire, types::v1 as types};
use crate::services::tilde::management::v1::IamService;
use connectrpc::{ConnectError, RequestContext, Response, ServiceRequest, ServiceResult};
use std::sync::Arc;

#[derive(Clone)]
pub struct Iam {
    pool: Pool,
}
/// Logs the database error and hides it from the caller.
fn failure(error: crate::database::DbError) -> ConnectError {
    tracing::error!(?error, "IAM operation failed");
    ConnectError::internal("IAM operation failed")
}
/// Page size: 50 by default, at most 100.
fn size(value: u32) -> usize {
    if value == 0 {
        50
    } else {
        value.min(100) as usize
    }
}
/// Trim to the page and return the cursor for the next one.
fn page<T>(rows: &mut Vec<T>, size: usize, cursor: impl Fn(&T) -> String) -> String {
    let more = rows.len() > size;
    rows.truncate(size);
    match rows.last() {
        Some(last) if more => cursor(last),
        _ => String::new(),
    }
}
fn resource(value: Option<types::Resource>) -> Result<Resource, ConnectError> {
    let value = value.ok_or_else(|| ConnectError::invalid_argument("Resource required"))?;
    let kind = match value.kind.as_known() {
        Some(types::ResourceKind::Agent) => Kind::Agent,
        _ => return Err(ConnectError::invalid_argument("Resource kind required")),
    };
    // An empty id addresses every resource of the kind.
    if value.id.is_empty() {
        return Ok(Resource::all(kind));
    }
    let id = super::parse_id(&value.id)?;
    if id.is_nil() {
        return Err(ConnectError::invalid_argument("Invalid resource ID"));
    }
    Ok(Resource { kind, id })
}
fn role_id(value: &str) -> Result<&str, ConnectError> {
    if value.is_empty() || value.len() > 200 {
        return Err(ConnectError::invalid_argument("Role required"));
    }
    Ok(value)
}
fn group_wire(row: db::GroupRow) -> wire::Group {
    wire::Group {
        source: match row.source.as_str() {
            "system" => wire::GroupSource::System,
            "external" => wire::GroupSource::External,
            _ => wire::GroupSource::Local,
        }
        .into(),
        id: row.id,
        name: row.name,
        created_at: crate::chat::audit::timestamp(row.created_at).into(),
        member_count: row.member_count.max(0) as u64,
        ..Default::default()
    }
}
fn slug(value: &str) -> Result<&str, ConnectError> {
    let valid = !value.is_empty()
        && value.len() <= 150
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
        && value
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-');
    valid
        .then_some(value)
        .ok_or_else(|| ConnectError::invalid_argument("Slug must be lowercase kebab-case"))
}

impl Iam {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }
    pub fn router(&self) -> axum::Router {
        crate::rpc::mount(
            connectrpc::Router::new().add_service(Arc::new(self.clone())),
            32 * 1024,
        )
    }
    async fn client(&self) -> Result<crate::database::Client, ConnectError> {
        self.pool.get().await.map_err(failure)
    }
    /// Membership of provider groups belongs to the provider; everyone is already a user.
    fn editable(group: &str) -> Result<(), ConnectError> {
        if group.starts_with(authz::EXTERNAL_PREFIX) || group == authz::USER_GROUP {
            return Err(ConnectError::failed_precondition(
                "Membership of this group is not managed here",
            ));
        }
        Ok(())
    }
}

impl IamService for Iam {
    async fn get_caller<'a>(
        &'a self,
        ctx: RequestContext,
        _: ServiceRequest<'_, wire::GetCallerRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::GetCallerResponse> + Send + use<'a>> {
        let access = authz::caller(&ctx)?;
        let mut creatable = Vec::new();
        if authz::require_create(&ctx, Kind::Agent).await.is_ok() {
            creatable.push(Kind::Agent.wire().into());
        }
        Response::ok(wire::GetCallerResponse {
            user_id: access.user.map(|id| id.to_string()),
            api_key_id: access.api_key.map(|id| id.to_string()),
            group_ids: access.groups.clone(),
            admin: access.admin,
            creatable,
            ..Default::default()
        })
    }
    async fn get_access<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, wire::GetAccessRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::GetAccessResponse> + Send + use<'a>> {
        authz::require_user(&ctx)?;
        let resource = resource(request.to_owned_message().resource.into_option())?;
        if resource.is_all() {
            return Err(ConnectError::invalid_argument("Resource ID required"));
        }
        authz::require(&ctx, resource, authz::Action::View).await?;
        let held = authz::authz(&ctx)?
            .actions(authz::caller(&ctx)?, resource)
            .await?;
        Response::ok(wire::GetAccessResponse {
            actions: held.iter().map(|a| a.name().to_string()).collect(),
            ..Default::default()
        })
    }
    async fn list_users<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, wire::ListUsersRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::ListUsersResponse> + Send + use<'a>> {
        authz::require_user(&ctx)?;
        if request.search.len() > 200 {
            return Err(ConnectError::invalid_argument("Search is too long"));
        }
        let after = (!request.page_token.is_empty())
            .then(|| super::parse_id(request.page_token))
            .transpose()?;
        let size = size(request.page_size);
        let mut rows = db::user_list(
            &self.client().await?,
            after,
            (size + 1) as i64,
            request.search.trim(),
        )
        .await
        .map_err(failure)?;
        let next_page_token = page(&mut rows, size, |row| row.id.to_string());
        Response::ok(wire::ListUsersResponse {
            users: rows
                .into_iter()
                .map(|row| wire::IamUser {
                    id: row.id.to_string(),
                    issuer: row.issuer,
                    subject: row.subject,
                    email: row.email,
                    display_name: row.display_name,
                    group_ids: row.group_ids,
                    ..Default::default()
                })
                .collect(),
            next_page_token,
            ..Default::default()
        })
    }
    async fn revoke_user_sessions<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, wire::RevokeUserSessionsRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::RevokeUserSessionsResponse> + Send + use<'a>>
    {
        authz::require_admin(&ctx)?;
        db::sessions_revoke(&self.client().await?, super::parse_id(request.user_id)?)
            .await
            .map_err(failure)?;
        Response::ok(wire::RevokeUserSessionsResponse::default())
    }
    async fn create_group<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, wire::CreateGroupRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::CreateGroupResponse> + Send + use<'a>> {
        authz::require_admin(&ctx)?;
        let name = request.name.trim();
        if name.is_empty() || name.chars().count() > 100 {
            return Err(ConnectError::invalid_argument(
                "Name must contain 1 to 100 characters",
            ));
        }
        let (prefix, source) = match request.source.as_known() {
            Some(wire::GroupSource::Local) => (authz::LOCAL_PREFIX, "local"),
            Some(wire::GroupSource::External) => (authz::EXTERNAL_PREFIX, "external"),
            _ => {
                return Err(ConnectError::invalid_argument(
                    "Group source must be local or external",
                ));
            }
        };
        let id = format!("{prefix}{}", slug(request.slug)?);
        let group = db::group_create(&self.client().await?, &id, name, source)
            .await
            .map_err(failure)?
            .ok_or_else(|| ConnectError::already_exists("Group already exists"))?;
        Response::ok(wire::CreateGroupResponse {
            group: group_wire(group).into(),
            ..Default::default()
        })
    }
    async fn list_groups<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, wire::ListGroupsRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::ListGroupsResponse> + Send + use<'a>> {
        authz::require_user(&ctx)?;
        if request.search.len() > 200 || request.page_token.len() > 200 {
            return Err(ConnectError::invalid_argument("Invalid list request"));
        }
        let size = size(request.page_size);
        let mut rows = db::group_list(
            &self.client().await?,
            (!request.page_token.is_empty()).then_some(request.page_token),
            (size + 1) as i64,
            request.search.trim(),
        )
        .await
        .map_err(failure)?;
        let next_page_token = page(&mut rows, size, |row| row.id.clone());
        Response::ok(wire::ListGroupsResponse {
            groups: rows.into_iter().map(group_wire).collect(),
            next_page_token,
            ..Default::default()
        })
    }
    async fn get_group<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, wire::GetGroupRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::GetGroupResponse> + Send + use<'a>> {
        let access = authz::require_user(&ctx)?;
        if !access.admin && !access.groups.iter().any(|g| g == request.id) {
            return Err(ConnectError::not_found("Group not found"));
        }
        let group = db::group_get(&self.client().await?, request.id)
            .await
            .map_err(failure)?
            .ok_or_else(|| ConnectError::not_found("Group not found"))?;
        Response::ok(wire::GetGroupResponse {
            group: group_wire(group).into(),
            ..Default::default()
        })
    }
    async fn delete_group<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, wire::DeleteGroupRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::DeleteGroupResponse> + Send + use<'a>> {
        authz::require_admin(&ctx)?;
        if request.id.starts_with(authz::SYSTEM_PREFIX) {
            return Err(ConnectError::failed_precondition(
                "System groups cannot be deleted",
            ));
        }
        if db::group_delete(&self.client().await?, request.id)
            .await
            .map_err(failure)?
            == 0
        {
            return Err(ConnectError::not_found("Group not found"));
        }
        Response::ok(wire::DeleteGroupResponse::default())
    }
    async fn list_group_members<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, wire::ListGroupMembersRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::ListGroupMembersResponse> + Send + use<'a>>
    {
        let access = authz::require_user(&ctx)?;
        if !access.admin && !access.groups.iter().any(|g| g == request.group_id) {
            return Err(ConnectError::not_found("Group not found"));
        }
        let after = (!request.page_token.is_empty())
            .then(|| super::parse_id(request.page_token))
            .transpose()?;
        let size = size(request.page_size);
        let mut rows = db::group_member_list(
            &self.client().await?,
            request.group_id,
            after,
            (size + 1) as i64,
        )
        .await
        .map_err(failure)?;
        let next_page_token = page(&mut rows, size, |row| row.user_id.to_string());
        Response::ok(wire::ListGroupMembersResponse {
            members: rows
                .into_iter()
                .map(|row| wire::GroupMember {
                    user_id: row.user_id.to_string(),
                    label: row.label,
                    ..Default::default()
                })
                .collect(),
            next_page_token,
            ..Default::default()
        })
    }
    async fn add_group_member<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, wire::AddGroupMemberRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::AddGroupMemberResponse> + Send + use<'a>>
    {
        authz::require_admin(&ctx)?;
        Self::editable(request.group_id)?;
        let member = super::parse_id(request.user_id)?;
        let client = self.client().await?;
        if !db::group_member_add(&client, request.group_id, member)
            .await
            .map_err(failure)?
            && db::group_get(&client, request.group_id)
                .await
                .map_err(failure)?
                .is_none()
        {
            return Err(ConnectError::not_found("Group not found"));
        }
        Response::ok(wire::AddGroupMemberResponse::default())
    }
    async fn remove_group_member<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, wire::RemoveGroupMemberRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::RemoveGroupMemberResponse> + Send + use<'a>>
    {
        let access = authz::require_admin(&ctx)?;
        Self::editable(request.group_id)?;
        let member = super::parse_id(request.user_id)?;
        // An administrator cannot lock themselves out; another administrator removes them.
        if request.group_id == authz::ADMIN_GROUP && access.user == Some(member) {
            return Err(ConnectError::failed_precondition(
                "Administrators cannot remove themselves",
            ));
        }
        db::group_member_remove(&self.client().await?, request.group_id, member)
            .await
            .map_err(failure)?;
        Response::ok(wire::RemoveGroupMemberResponse::default())
    }
    async fn list_roles<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, wire::ListRolesRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::ListRolesResponse> + Send + use<'a>> {
        authz::require_user(&ctx)?;
        let resource = resource(request.to_owned_message().resource.into_option())?;
        if resource.is_all() {
            authz::require_admin(&ctx)?;
        } else {
            authz::require(&ctx, resource, authz::Action::View).await?;
        }
        let roles = db::role_list(&self.client().await?, resource)
            .await
            .map_err(failure)?;
        Response::ok(wire::ListRolesResponse {
            roles,
            ..Default::default()
        })
    }
    async fn list_role_assignments<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, wire::ListRoleAssignmentsRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::ListRoleAssignmentsResponse> + Send + use<'a>>
    {
        authz::require_user(&ctx)?;
        let resource = resource(request.to_owned_message().resource.into_option())?;
        if resource.is_all() {
            authz::require_admin(&ctx)?;
        } else {
            authz::require(&ctx, resource, authz::Action::View).await?;
        }
        let rows = db::role_assignments(&self.client().await?, resource)
            .await
            .map_err(failure)?;
        // Rows arrive ordered by principal; fold each principal's roles into one assignment.
        let mut assignments: Vec<types::RoleAssignment> = Vec::new();
        let mut last: Option<Grantee> = None;
        for row in rows {
            if last.as_ref() != Some(&row.grantee) {
                assignments.push(types::RoleAssignment {
                    principal: row.grantee.wire().into(),
                    label: row.label,
                    created_at: crate::chat::audit::timestamp(row.created_at).into(),
                    ..Default::default()
                });
                last = Some(row.grantee);
            }
            assignments
                .last_mut()
                .expect("pushed above")
                .roles
                .push(row.role);
        }
        Response::ok(wire::ListRoleAssignmentsResponse {
            assignments,
            ..Default::default()
        })
    }
    async fn assign_role<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, wire::AssignRoleRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::AssignRoleResponse> + Send + use<'a>> {
        let access = authz::require_user(&ctx)?;
        let request = request.to_owned_message();
        let role = role_id(&request.role_id)?;
        let grantee = Grantee::parse(request.principal.into_option())?;
        authz::authz(&ctx)?.require_reach(access, role).await?;
        if !db::role_member_put(&self.client().await?, role, &grantee, access)
            .await
            .map_err(failure)?
        {
            return Err(ConnectError::not_found("Principal not found"));
        }
        Response::ok(wire::AssignRoleResponse::default())
    }
    async fn revoke_role<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, wire::RevokeRoleRequest>,
    ) -> ServiceResult<impl connectrpc::Encodable<wire::RevokeRoleResponse> + Send + use<'a>> {
        let access = authz::require_user(&ctx)?;
        let request = request.to_owned_message();
        let role = role_id(&request.role_id)?;
        let grantee = Grantee::parse(request.principal.into_option())?;
        authz::authz(&ctx)?.require_reach(access, role).await?;
        db::role_member_delete(&self.client().await?, role, &grantee)
            .await
            .map_err(failure)?;
        Response::ok(wire::RevokeRoleResponse::default())
    }
}
