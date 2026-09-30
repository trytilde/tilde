//! Management CRUD for the Skills pages and the runtime surface agents read skills through.
use super::{FileBody, Kind as SourceKind, Skill, Skills, Upload, db};
use crate::error::Error;
use crate::iam::capabilities::Capability;
use crate::proto::tilde::{
    management::v1 as management, runtime::v1 as runtime, types::v1 as types,
};
use crate::services::tilde::management::v1::SkillService as ManagementSkillService;
use crate::services::tilde::runtime::v1::SkillService as RuntimeSkillService;
use connectrpc::{
    ConnectError, Encodable, RequestContext, Response, ServiceRequest, ServiceResult,
};
use std::sync::Arc;
use uuid::Uuid;

fn id(value: &str) -> Result<Uuid, Error> {
    Uuid::parse_str(value).map_err(|_| Error::Invalid("Invalid UUID".into()))
}
fn timestamp(date: chrono::DateTime<chrono::Utc>) -> buffa_types::google::protobuf::Timestamp {
    buffa_types::google::protobuf::Timestamp {
        seconds: date.timestamp(),
        nanos: date.timestamp_subsec_nanos() as i32,
        ..Default::default()
    }
}
fn kind_wire(kind: &str) -> types::SkillSourceKind {
    match kind {
        "catalog" => types::SkillSourceKind::Catalog,
        "git" => types::SkillSourceKind::Git,
        "editor" => types::SkillSourceKind::Editor,
        "bundled" => types::SkillSourceKind::Bundled,
        _ => types::SkillSourceKind::Unspecified,
    }
}
fn source_wire(s: db::SourceRow) -> types::SkillSource {
    types::SkillSource {
        id: s.id.to_string(),
        slug: s.slug,
        name: s.name,
        kind: kind_wire(&s.kind).into(),
        catalog_group: s.catalog_group.unwrap_or_default(),
        repository_url: s.repository_url.unwrap_or_default(),
        git_ref: s.git_ref.unwrap_or_default(),
        git_path: s.git_path,
        commit_sha: s.commit_sha.unwrap_or_default(),
        sync_error: s.sync_error.unwrap_or_default(),
        skill_count: s.skill_count.max(0) as u32,
        created_at: timestamp(s.created_at).into(),
        ..Default::default()
    }
}
fn version_wire(v: db::VersionRow, files: Vec<types::SkillFile>) -> types::SkillVersion {
    types::SkillVersion {
        id: v.id.to_string(),
        skill_id: v.skill_id.to_string(),
        number: v.number.max(0) as u32,
        description: v.description,
        created_at: timestamp(v.created_at).into(),
        commit_sha: v.commit_sha.unwrap_or_default(),
        files,
        ..Default::default()
    }
}
fn skill_wire(s: Skill) -> types::Skill {
    types::Skill {
        id: s.row.id.to_string(),
        source_id: s.row.source_id.to_string(),
        source_name: s.row.source_name,
        source_kind: kind_wire(&s.row.source_kind).into(),
        name: s.row.name,
        created_at: timestamp(s.row.created_at).into(),
        latest: s.latest.map(|v| version_wire(v, vec![])).into(),
        ..Default::default()
    }
}
fn group_wire(entry: super::CatalogEntry) -> types::CatalogGroup {
    let group = entry.group;
    let (repository_url, branch) = match &group.origin {
        super::catalog::Origin::Repository(r) => (r.url, r.branch),
        super::catalog::Origin::Builtin(_) => ("", ""),
    };
    types::CatalogGroup {
        id: group.id.into(),
        name: group.name.into(),
        description: group.description.into(),
        skills: entry
            .skills
            .into_iter()
            .map(|s| types::CatalogSkill {
                name: s.name,
                description: s.description,
                path: s.path,
                ..Default::default()
            })
            .collect(),
        source_id: entry.source.map(|s| s.to_string()).unwrap_or_default(),
        category: group.category.into(),
        icon_url: group.icon_url.into(),
        repository_url: repository_url.into(),
        branch: branch.into(),
        ..Default::default()
    }
}
/// Bytes or text become new files; `keep` reuses the current version's file at that path, or
/// at `keep_path` when it was renamed or moved.
fn uploads(files: Vec<types::SkillFile>) -> Vec<Upload> {
    files
        .into_iter()
        .map(|f| {
            if f.keep {
                Upload::Keep {
                    path: f.path,
                    from: (!f.keep_path.is_empty()).then_some(f.keep_path),
                }
            } else if !f.data.is_empty() {
                Upload::Data {
                    path: f.path,
                    data: f.data.to_vec(),
                    executable: f.executable,
                }
            } else {
                Upload::Data {
                    path: f.path,
                    data: f.content.into_bytes(),
                    executable: f.executable,
                }
            }
        })
        .collect()
}
struct Management(Skills);
pub fn management_router(skills: Skills) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Management(skills))),
        96 * 1024 * 1024,
    )
}
impl ManagementSkillService for Management {
    async fn list_catalog<'a>(
        &'a self,
        _: RequestContext,
        _: ServiceRequest<'_, management::ListCatalogRequest>,
    ) -> ServiceResult<impl Encodable<management::ListCatalogResponse> + Send + use<'a>> {
        Response::ok(management::ListCatalogResponse {
            groups: self
                .0
                .catalog()
                .await?
                .into_iter()
                .map(group_wire)
                .collect(),
            ..Default::default()
        })
    }
    async fn get_catalog_group<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::GetCatalogGroupRequest>,
    ) -> ServiceResult<impl Encodable<management::GetCatalogGroupResponse> + Send + use<'a>> {
        Response::ok(management::GetCatalogGroupResponse {
            group: group_wire(self.0.catalog_group(request.id).await?).into(),
            ..Default::default()
        })
    }
    async fn list_skill_sources<'a>(
        &'a self,
        _: RequestContext,
        _: ServiceRequest<'_, management::ListSkillSourcesRequest>,
    ) -> ServiceResult<impl Encodable<management::ListSkillSourcesResponse> + Send + use<'a>> {
        let listed = self.0.sources().await?;
        let ids: Vec<Uuid> = listed.iter().map(|s| s.id).collect();
        let mut agents = self.0.group_agents(&ids).await?;
        Response::ok(management::ListSkillSourcesResponse {
            sources: listed
                .into_iter()
                .map(|source| {
                    let agent_ids = agents.remove(&source.id).unwrap_or_default();
                    types::SkillSource {
                        agent_ids: agent_ids.iter().map(Uuid::to_string).collect(),
                        ..source_wire(source)
                    }
                })
                .collect(),
            ..Default::default()
        })
    }
    async fn get_skill_source<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::GetSkillSourceRequest>,
    ) -> ServiceResult<impl Encodable<management::GetSkillSourceResponse> + Send + use<'a>> {
        let source = id(request.id)?;
        let agents = self.0.source_agents(source).await?;
        Response::ok(management::GetSkillSourceResponse {
            source: source_wire(self.0.source(source).await?).into(),
            skills: self
                .0
                .skills(Some(source))
                .await?
                .into_iter()
                .map(skill_wire)
                .collect(),
            agent_ids: agents.iter().map(Uuid::to_string).collect(),
            connection_ids: self
                .0
                .source_connections(source)
                .await?
                .into_iter()
                .map(|c| c.to_string())
                .collect(),
            ..Default::default()
        })
    }
    async fn enable_catalog_group<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::EnableCatalogGroupRequest>,
    ) -> ServiceResult<impl Encodable<management::EnableCatalogGroupResponse> + Send + use<'a>>
    {
        let (source, _) = self.0.enable_catalog(request.group).await?;
        Response::ok(management::EnableCatalogGroupResponse {
            source: source_wire(self.0.source(source).await?).into(),
            ..Default::default()
        })
    }
    async fn add_git_source<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::AddGitSourceRequest>,
    ) -> ServiceResult<impl Encodable<management::AddGitSourceResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let source = self
            .0
            .add_git(&r.name, &r.repository_url, &r.git_ref, &r.git_path)
            .await?;
        Response::ok(management::AddGitSourceResponse {
            source: source_wire(self.0.source(source).await?).into(),
            ..Default::default()
        })
    }
    async fn create_editor_source<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::CreateEditorSourceRequest>,
    ) -> ServiceResult<impl Encodable<management::CreateEditorSourceResponse> + Send + use<'a>>
    {
        let source = self.0.create_editor(request.name).await?;
        Response::ok(management::CreateEditorSourceResponse {
            source: source_wire(self.0.source(source).await?).into(),
            ..Default::default()
        })
    }
    async fn sync_skill_source<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::SyncSkillSourceRequest>,
    ) -> ServiceResult<impl Encodable<management::SyncSkillSourceResponse> + Send + use<'a>> {
        let source = id(request.id)?;
        self.0.sync(source).await?;
        Response::ok(management::SyncSkillSourceResponse {
            source: source_wire(self.0.source(source).await?).into(),
            ..Default::default()
        })
    }
    async fn delete_skill_source<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::DeleteSkillSourceRequest>,
    ) -> ServiceResult<impl Encodable<management::DeleteSkillSourceResponse> + Send + use<'a>> {
        let source = id(request.id)?;
        self.0.delete_source(source).await?;
        Response::ok(management::DeleteSkillSourceResponse::default())
    }
    async fn list_skills<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::ListSkillsRequest>,
    ) -> ServiceResult<impl Encodable<management::ListSkillsResponse> + Send + use<'a>> {
        let source = if request.source_id.is_empty() {
            None
        } else {
            Some(id(request.source_id)?)
        };
        let listed = self.0.skills(source).await?;
        let ids: Vec<Uuid> = listed.iter().map(|s| s.row.id).collect();
        let mut agents = self.0.skill_agents(&ids).await?;
        Response::ok(management::ListSkillsResponse {
            skills: listed
                .into_iter()
                .map(|skill| {
                    let agent_ids = agents.remove(&skill.row.id).unwrap_or_default();
                    types::Skill {
                        agent_ids: agent_ids.iter().map(Uuid::to_string).collect(),
                        ..skill_wire(skill)
                    }
                })
                .collect(),
            ..Default::default()
        })
    }
    async fn get_skill<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::GetSkillRequest>,
    ) -> ServiceResult<impl Encodable<management::GetSkillResponse> + Send + use<'a>> {
        let skill = self.0.skill(id(request.id)?).await?;
        let versions = self.0.versions(skill.row.id).await?;
        Response::ok(management::GetSkillResponse {
            skill: skill_wire(skill).into(),
            versions: versions
                .into_iter()
                .map(|v| version_wire(v, vec![]))
                .collect(),
            ..Default::default()
        })
    }
    async fn get_skill_version<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::GetSkillVersionRequest>,
    ) -> ServiceResult<impl Encodable<management::GetSkillVersionResponse> + Send + use<'a>> {
        let version = self.0.version(id(request.id)?).await?;
        let files = version
            .files
            .into_iter()
            .map(|f| types::SkillFile {
                path: f.path,
                text: f.content.is_some(),
                content: f.content.unwrap_or_default(),
                media_type: f.media_type,
                size_bytes: f.size,
                sha256: hex::encode(f.digest),
                executable: f.executable,
                download_url: f.download_url.unwrap_or_default(),
                ..Default::default()
            })
            .collect();
        Response::ok(management::GetSkillVersionResponse {
            version: version_wire(version.row, files).into(),
            ..Default::default()
        })
    }
    async fn create_skill<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::CreateSkillRequest>,
    ) -> ServiceResult<impl Encodable<management::CreateSkillResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let source = id(&r.source_id)?;
        let (skill, _) = self
            .0
            .write(source, &r.name, uploads(r.files), "", true)
            .await?;
        Response::ok(management::CreateSkillResponse {
            skill: skill_wire(self.0.skill(skill).await?).into(),
            ..Default::default()
        })
    }
    async fn update_skill<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::UpdateSkillRequest>,
    ) -> ServiceResult<impl Encodable<management::UpdateSkillResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let skill = self.0.skill(id(&r.id)?).await?;
        self.0.update(skill.row.id, uploads(r.files)).await?;
        Response::ok(management::UpdateSkillResponse {
            skill: skill_wire(self.0.skill(skill.row.id).await?).into(),
            ..Default::default()
        })
    }
    async fn delete_skill<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::DeleteSkillRequest>,
    ) -> ServiceResult<impl Encodable<management::DeleteSkillResponse> + Send + use<'a>> {
        let skill = self.0.skill(id(request.id)?).await?;
        self.0.delete_skill(skill.row.id).await?;
        Response::ok(management::DeleteSkillResponse::default())
    }
    async fn list_agent_skills<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::ListAgentSkillsRequest>,
    ) -> ServiceResult<impl Encodable<management::ListAgentSkillsResponse> + Send + use<'a>> {
        let agent = id(request.agent_id)?;
        let (sources, skills) = self.0.agent_skills(agent).await?;
        let mut whole: Vec<Uuid> = sources.iter().map(|s| s.id).collect();
        let mut connections = Vec::new();
        for c in self.0.agent_connections(agent).await? {
            let sources = self.0.connection_sources(c.id).await?;
            whole.extend(sources.iter().map(|s| s.id));
            connections.push(management::ConnectionSkills {
                connection_id: c.id.to_string(),
                connection_name: c.name,
                provider_id: c.provider_id,
                status: c.status,
                sources: sources.into_iter().map(source_wire).collect(),
                ..Default::default()
            });
        }
        Response::ok(management::ListAgentSkillsResponse {
            sources: sources.into_iter().map(source_wire).collect(),
            skills: skills.into_iter().map(skill_wire).collect(),
            connections,
            disabled_source_ids: self
                .0
                .disabled_sources(agent)
                .await?
                .iter()
                .map(Uuid::to_string)
                .collect(),
            disabled_skill_ids: self
                .0
                .excluded_skills(agent)
                .await?
                .iter()
                .map(Uuid::to_string)
                .collect(),
            group_skills: self
                .0
                .group_skills(&whole)
                .await?
                .into_iter()
                .map(skill_wire)
                .collect(),
            ..Default::default()
        })
    }
    async fn assign_skill_source<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::AssignSkillSourceRequest>,
    ) -> ServiceResult<impl Encodable<management::AssignSkillSourceResponse> + Send + use<'a>> {
        let (agent, source) = (id(request.agent_id)?, id(request.source_id)?);
        self.0.assign_source(agent, source).await?;
        Response::ok(management::AssignSkillSourceResponse::default())
    }
    async fn unassign_skill_source<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::UnassignSkillSourceRequest>,
    ) -> ServiceResult<impl Encodable<management::UnassignSkillSourceResponse> + Send + use<'a>>
    {
        let (agent, source) = (id(request.agent_id)?, id(request.source_id)?);
        self.0.unassign_source(agent, source).await?;
        Response::ok(management::UnassignSkillSourceResponse::default())
    }
    async fn set_skill_source_enabled<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::SetSkillSourceEnabledRequest>,
    ) -> ServiceResult<impl Encodable<management::SetSkillSourceEnabledResponse> + Send + use<'a>>
    {
        let (agent, source) = (id(request.agent_id)?, id(request.source_id)?);
        self.0.enable_source(agent, source, request.enabled).await?;
        Response::ok(management::SetSkillSourceEnabledResponse::default())
    }
    async fn set_skill_enabled<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::SetSkillEnabledRequest>,
    ) -> ServiceResult<impl Encodable<management::SetSkillEnabledResponse> + Send + use<'a>> {
        let (agent, skill) = (id(request.agent_id)?, id(request.skill_id)?);
        self.0.enable_skill(agent, skill, request.enabled).await?;
        Response::ok(management::SetSkillEnabledResponse::default())
    }
    async fn unassign_skill<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::UnassignSkillRequest>,
    ) -> ServiceResult<impl Encodable<management::UnassignSkillResponse> + Send + use<'a>> {
        let (agent, skill) = (id(request.agent_id)?, id(request.skill_id)?);
        self.0.unassign_skill(agent, skill).await?;
        Response::ok(management::UnassignSkillResponse::default())
    }
    async fn link_connection_skill_source<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::LinkConnectionSkillSourceRequest>,
    ) -> ServiceResult<impl Encodable<management::LinkConnectionSkillSourceResponse> + Send + use<'a>>
    {
        let (connection, source) = (id(request.connection_id)?, id(request.source_id)?);
        self.0.link_source(connection, source, true).await?;
        Response::ok(management::LinkConnectionSkillSourceResponse::default())
    }
    async fn unlink_connection_skill_source<'a>(
        &'a self,
        _: RequestContext,
        request: ServiceRequest<'_, management::UnlinkConnectionSkillSourceRequest>,
    ) -> ServiceResult<
        impl Encodable<management::UnlinkConnectionSkillSourceResponse> + Send + use<'a>,
    > {
        let (connection, source) = (id(request.connection_id)?, id(request.source_id)?);
        self.0.link_source(connection, source, false).await?;
        Response::ok(management::UnlinkConnectionSkillSourceResponse::default())
    }
}

struct Runtime {
    skills: Skills,
    chat: crate::chat::Chat,
}
pub fn runtime_router(skills: Skills, chat: crate::chat::Chat) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Runtime { skills, chat })),
        96 * 1024 * 1024,
    )
}
/// The agent an RPC addresses: itself when the id is empty, otherwise one the capability
/// reaches. Reads of itself need nothing; everything else needs the capability.
fn target(
    scope: &crate::chat::Scope,
    agent_id: &str,
    capability: Capability,
    self_free: bool,
) -> Result<Uuid, ConnectError> {
    let own = scope.agent_id.to_string();
    if agent_id.is_empty() || agent_id == own {
        if !self_free {
            scope.capabilities.require(capability, &own)?;
        }
        return Ok(scope.agent_id);
    }
    let agent = id(agent_id)?;
    scope.capabilities.require(capability, agent_id)?;
    Ok(agent)
}
pub(crate) fn summary_wire(s: super::RuntimeSkill) -> runtime::SkillSummary {
    runtime::SkillSummary {
        deployed: s.deployed,
        name: s.name,
        source: s.source,
        description: s.description,
        version_id: s.version_id.to_string(),
        files: s
            .files
            .into_iter()
            .map(
                |(path, media_type, size, executable)| runtime::SkillFileInfo {
                    path,
                    media_type,
                    size_bytes: size,
                    executable,
                    ..Default::default()
                },
            )
            .collect(),
        ..Default::default()
    }
}
impl Runtime {
    async fn scope(&self, ctx: &RequestContext) -> Result<crate::chat::Scope, ConnectError> {
        crate::chat::rpc::invocation_scope(&self.chat, ctx).await
    }
    /// A source addressed by slug, when the capability reaches it.
    async fn editable_source(
        &self,
        scope: &crate::chat::Scope,
        slug: &str,
    ) -> Result<(Uuid, SourceKind), ConnectError> {
        let (source, kind) = self.skills.source_by_slug(slug).await?;
        scope
            .capabilities
            .require(Capability::SkillsEdit, &source.to_string())?;
        Ok((source, kind))
    }
}
/// Whether the agent may see and assign the source's skills; `skills.edit` implies `skills.read`.
fn readable(scope: &crate::chat::Scope, source: Uuid) -> bool {
    let id = source.to_string();
    scope.capabilities.permits(Capability::SkillsRead, &id)
        || scope.capabilities.permits(Capability::SkillsEdit, &id)
}
fn require_readable(scope: &crate::chat::Scope, source: Uuid) -> Result<(), ConnectError> {
    if readable(scope, source) {
        Ok(())
    } else {
        Err(ConnectError::permission_denied(
            "Capability does not permit this operation",
        ))
    }
}
impl RuntimeSkillService for Runtime {
    async fn list_skills<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, runtime::ListSkillsRequest>,
    ) -> ServiceResult<impl Encodable<runtime::ListSkillsResponse> + Send + use<'a>> {
        let scope = self.scope(&ctx).await?;
        let agent = target(&scope, request.agent_id, Capability::AgentsRead, true)?;
        Response::ok(runtime::ListSkillsResponse {
            skills: self
                .skills
                .for_agent(agent, (agent == scope.agent_id).then_some(scope.id))
                .await?
                .into_iter()
                .map(summary_wire)
                .collect(),
            ..Default::default()
        })
    }
    async fn read_skill_file<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, runtime::ReadSkillFileRequest>,
    ) -> ServiceResult<impl Encodable<runtime::ReadSkillFileResponse> + Send + use<'a>> {
        let scope = self.scope(&ctx).await?;
        let agent = target(&scope, request.agent_id, Capability::AgentsRead, true)?;
        let (content, download_url, media_type) = match self
            .skills
            .read(
                agent,
                (agent == scope.agent_id).then_some(scope.id),
                request.name,
                request.path,
            )
            .await?
        {
            FileBody::Text(text, media) => (text, String::new(), media),
            FileBody::Url(url, media) => (String::new(), url, media),
        };
        Response::ok(runtime::ReadSkillFileResponse {
            content,
            download_url,
            media_type,
            ..Default::default()
        })
    }
    async fn list_skill_sources<'a>(
        &'a self,
        ctx: RequestContext,
        _: ServiceRequest<'_, runtime::ListSkillSourcesRequest>,
    ) -> ServiceResult<impl Encodable<runtime::ListSkillSourcesResponse> + Send + use<'a>> {
        let scope = self.scope(&ctx).await?;
        let sources = self
            .skills
            .catalogue()
            .await?
            .into_iter()
            .filter(|(source, _)| readable(&scope, source.id))
            .map(|(source, skills)| runtime::SourceSummary {
                slug: source.slug.clone(),
                name: source.name,
                kind: kind_wire(&source.kind).into(),
                skills: skills
                    .into_iter()
                    .map(|s| runtime::SkillSummary {
                        name: s.row.name,
                        source: source.slug.clone(),
                        description: s
                            .latest
                            .as_ref()
                            .map(|v| v.description.clone())
                            .unwrap_or_default(),
                        version_id: s.latest.map(|v| v.id.to_string()).unwrap_or_default(),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            })
            .collect();
        Response::ok(runtime::ListSkillSourcesResponse {
            sources,
            ..Default::default()
        })
    }
    /// Giving skills needs edit_skills on the receiving agent and skills.read on their source.
    async fn assign_skill<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, runtime::AssignSkillRequest>,
    ) -> ServiceResult<impl Encodable<runtime::AssignSkillResponse> + Send + use<'a>> {
        let scope = self.scope(&ctx).await?;
        let agent = target(
            &scope,
            request.agent_id,
            Capability::AgentsEditSkills,
            false,
        )?;
        match (request.source.is_empty(), request.skill.is_empty()) {
            (false, true) => {
                let (source, _) = self.skills.source_by_slug(request.source).await?;
                require_readable(&scope, source)?;
                // An agent naming a whole group means every skill in it, switched on.
                self.skills.assign_source(agent, source).await?;
                self.skills.enable_source(agent, source, true).await?;
            }
            (true, false) => {
                let (source, skill) = self.skills.skill_by_address(request.skill).await?;
                require_readable(&scope, source)?;
                self.skills.assign_skill(agent, skill).await?;
            }
            _ => return Err(ConnectError::invalid_argument("Name a source or a skill")),
        }
        Response::ok(runtime::AssignSkillResponse::default())
    }
    async fn unassign_skill<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, runtime::UnassignSkillRequest>,
    ) -> ServiceResult<impl Encodable<runtime::UnassignSkillResponse> + Send + use<'a>> {
        let scope = self.scope(&ctx).await?;
        let agent = target(
            &scope,
            request.agent_id,
            Capability::AgentsEditSkills,
            false,
        )?;
        match (request.source.is_empty(), request.skill.is_empty()) {
            (false, true) => {
                let (source, _) = self.skills.source_by_slug(request.source).await?;
                self.skills.unassign_source(agent, source).await?;
            }
            (true, false) => {
                let (_, skill) = self.skills.skill_by_address(request.skill).await?;
                self.skills.unassign_skill(agent, skill).await?;
            }
            _ => return Err(ConnectError::invalid_argument("Name a source or a skill")),
        }
        Response::ok(runtime::UnassignSkillResponse::default())
    }
    /// Authoring is skills.edit on the editor source; the agent then assigns what it wrote.
    async fn write_skill<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, runtime::WriteSkillRequest>,
    ) -> ServiceResult<impl Encodable<runtime::WriteSkillResponse> + Send + use<'a>> {
        let scope = self.scope(&ctx).await?;
        let r = request.to_owned_message();
        let (source, _) = self.editable_source(&scope, &r.source).await?;
        let (_, written) = self
            .skills
            .write(source, &r.name, uploads(r.files), &r.message, false)
            .await?;
        Response::ok(runtime::WriteSkillResponse {
            version_id: written.version.to_string(),
            number: written.number.max(0) as u32,
            created: written.created,
            ..Default::default()
        })
    }
    async fn sync_skill_source<'a>(
        &'a self,
        ctx: RequestContext,
        request: ServiceRequest<'_, runtime::SyncSkillSourceRequest>,
    ) -> ServiceResult<impl Encodable<runtime::SyncSkillSourceResponse> + Send + use<'a>> {
        let scope = self.scope(&ctx).await?;
        let (source, _) = self.editable_source(&scope, request.source).await?;
        self.skills.sync(source).await?;
        let row = self.skills.source(source).await?;
        Response::ok(runtime::SyncSkillSourceResponse {
            commit_sha: row.commit_sha.unwrap_or_default(),
            sync_error: row.sync_error.unwrap_or_default(),
            ..Default::default()
        })
    }
}
