//! The memory management service.
use super::{Actor, Change, Draft, Filter, Memory, Person, Source, db};
use crate::chat::audit::timestamp;
use crate::error::Error;
use crate::proto::tilde::management::v1 as management;
use crate::services::tilde::management::v1::MemoryService;
use buffa_types::google::protobuf::Timestamp;
use chrono::{DateTime, Utc};
use connectrpc::{
    ConnectError, Encodable, RequestContext, Response, ServiceRequest, ServiceResult,
};
use std::sync::Arc;
use uuid::Uuid;

pub fn router(memory: Memory) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Management(memory))),
        256 * 1024,
    )
}
struct Management(Memory);
impl Management {
    /// Memories on the wire, each with how it was found and what a synthesized one was derived
    /// from.
    async fn wire(
        &self,
        _ctx: &RequestContext,
        rows: Vec<(db::MemoryRow, Vec<&str>)>,
    ) -> Result<Vec<management::Memory>, ConnectError> {
        let ids: Vec<Uuid> = rows.iter().map(|(m, _)| m.id).collect();
        let mut evidence = self.0.evidence(&ids).await?;
        Ok(rows
            .into_iter()
            .map(|(m, matched)| {
                let id = m.id;
                management::Memory {
                    matched: matched.into_iter().map(|h| match_wire(h).into()).collect(),
                    evidence: evidence
                        .remove(&id)
                        .unwrap_or_default()
                        .into_iter()
                        .map(evidence_wire)
                        .collect(),
                    ..memory_wire(m)
                }
            })
            .collect())
    }
}

fn id(value: &str) -> Result<Uuid, Error> {
    Uuid::parse_str(value).map_err(|_| Error::Invalid("Invalid UUID".into()))
}
fn time(t: Option<Timestamp>) -> Result<Option<DateTime<Utc>>, Error> {
    t.map(|t| {
        DateTime::from_timestamp(t.seconds, t.nanos.max(0) as u32)
            .ok_or_else(|| Error::Invalid("Invalid timestamp".into()))
    })
    .transpose()
}
fn kind_text(kind: buffa::EnumValue<management::MemoryKind>) -> Result<Option<String>, Error> {
    use management::MemoryKind as K;
    Ok(match kind.as_known() {
        Some(K::Unspecified) => None,
        Some(K::Profile) => Some("profile"),
        Some(K::Preferences) => Some("preferences"),
        Some(K::Entities) => Some("entities"),
        Some(K::Events) => Some("events"),
        Some(K::Cases) => Some("cases"),
        Some(K::Patterns) => Some("patterns"),
        Some(K::Tools) => Some("tools"),
        Some(K::Skills) => Some("skills"),
        None => return Err(Error::Invalid("Unknown memory kind".into())),
    }
    .map(str::to_owned))
}
fn kind_wire(kind: &str) -> management::MemoryKind {
    use management::MemoryKind as K;
    match kind {
        "profile" => K::Profile,
        "preferences" => K::Preferences,
        "entities" => K::Entities,
        "events" => K::Events,
        "cases" => K::Cases,
        "patterns" => K::Patterns,
        "tools" => K::Tools,
        "skills" => K::Skills,
        _ => K::Unspecified,
    }
}
fn scope_wire(scope: &str) -> management::MemoryBankScope {
    match scope {
        "per_person" => management::MemoryBankScope::PerPerson,
        _ => management::MemoryBankScope::Shared,
    }
}
fn access_text(access: buffa::EnumValue<management::MemoryAccess>) -> Result<&'static str, Error> {
    match access.as_known() {
        Some(management::MemoryAccess::Read) => Ok("read"),
        Some(management::MemoryAccess::ReadWrite) => Ok("read_write"),
        _ => Err(Error::Invalid("Access is READ or READ_WRITE".into())),
    }
}
fn access_wire(access: &str) -> management::MemoryAccess {
    match access {
        "read_write" => management::MemoryAccess::ReadWrite,
        _ => management::MemoryAccess::Read,
    }
}
/// Unspecified means SHARED.
fn scope_text(scope: buffa::EnumValue<management::MemoryBankScope>) -> Result<&'static str, Error> {
    use management::MemoryBankScope as S;
    match scope.as_known() {
        Some(S::Unspecified | S::Shared) => Ok("shared"),
        Some(S::PerPerson) => Ok("per_person"),
        None => Err(Error::Invalid("Unknown memory bank scope".into())),
    }
}
fn bank_wire(b: db::BankRow) -> management::MemoryBank {
    management::MemoryBank {
        id: b.id.to_string(),
        name: b.name,
        description: b.description,
        scope: scope_wire(&b.scope).into(),
        agent_id: b.agent_id.map(|a| a.to_string()),
        links: b
            .agent_ids
            .iter()
            .zip(&b.link_access)
            .map(|(agent, access)| management::MemoryBankLink {
                agent_id: agent.to_string(),
                access: access_wire(access).into(),
                ..Default::default()
            })
            .collect(),
        memory_count: b.memory_count as u32,
        pending_embeddings: b.pending_embeddings as u32,
        created_at: timestamp(b.created_at).into(),
        ..Default::default()
    }
}
fn link_wire(l: db::AgentBankRow) -> management::AgentMemoryBank {
    management::AgentMemoryBank {
        agent_id: l.agent_id.to_string(),
        bank_id: l.bank_id.to_string(),
        bank_name: l.bank_name,
        access: access_wire(&l.access).into(),
        own: l.own,
        scope: scope_wire(&l.scope).into(),
        ..Default::default()
    }
}
fn origin_wire(origin: &str) -> management::MemoryOrigin {
    match origin {
        "owner" => management::MemoryOrigin::Owner,
        "synthesized" => management::MemoryOrigin::Synthesized,
        _ => management::MemoryOrigin::Explicit,
    }
}
fn actor_wire(actor: &str) -> management::MemoryActor {
    match actor {
        "agent" => management::MemoryActor::Agent,
        "synthesizer" => management::MemoryActor::Synthesizer,
        _ => management::MemoryActor::Person,
    }
}
/// Unspecified means any.
fn origin_text(
    origin: buffa::EnumValue<management::MemoryOrigin>,
) -> Result<Option<String>, Error> {
    match origin.as_known() {
        Some(management::MemoryOrigin::Unspecified) => Ok(None),
        Some(management::MemoryOrigin::Explicit) => Ok(Some("explicit".into())),
        Some(management::MemoryOrigin::Owner) => Ok(Some("owner".into())),
        Some(management::MemoryOrigin::Synthesized) => Ok(Some("synthesized".into())),
        None => Err(Error::Invalid("Unknown memory origin".into())),
    }
}
fn match_wire(how: &str) -> management::MemoryMatch {
    match how {
        "meaning" => management::MemoryMatch::Meaning,
        _ => management::MemoryMatch::Keywords,
    }
}
fn operation_wire(operation: &str) -> management::MemoryOperation {
    use management::MemoryOperation as O;
    match operation {
        "create" => O::Create,
        "update" => O::Update,
        "supersede" => O::Supersede,
        "forget" => O::Forget,
        _ => O::Unspecified,
    }
}
fn memory_wire(m: db::MemoryRow) -> management::Memory {
    management::Memory {
        id: m.id.to_string(),
        bank_id: m.bank_id.to_string(),
        kind: kind_wire(&m.kind).into(),
        path: m.path,
        title: m.title,
        content: m.content,
        valid_from: m.valid_from.map(timestamp).into(),
        valid_to: m.valid_to.map(timestamp).into(),
        importance: m.importance,
        origin: origin_wire(&m.origin).into(),
        identity_id: m.identity_id.map(|i| i.to_string()),
        written_by_agent_id: m.written_by_agent_id.map(|a| a.to_string()),
        supersedes_id: m.supersedes_id.map(|s| s.to_string()),
        superseded_by: m.superseded_by.map(|s| s.to_string()),
        embedded: m.embedded,
        thread_id: m.thread_id.map(|t| t.to_string()),
        message_id: m.message_id.map(|m| m.to_string()),
        created_at: timestamp(m.created_at).into(),
        updated_at: timestamp(m.updated_at).into(),
        ..Default::default()
    }
}
fn settings_wire(s: db::SettingsRow) -> management::EmbeddingSettings {
    management::EmbeddingSettings {
        connection_id: s.connection_id.to_string(),
        model: s.model,
        dimensions: s.dims as u32,
        ..Default::default()
    }
}
fn evidence_wire(e: db::EvidenceRow) -> management::MemoryEvidence {
    management::MemoryEvidence {
        thread_id: e.thread_id.map(|t| t.to_string()),
        message_id: e.message_id.map(|m| m.to_string()),
        agent_id: e.agent_id.map(|a| a.to_string()),
        page_id: e.page_id.map(|p| p.to_string()),
        revision: e.revision.map(|r| r as u32),
        wiki_id: e.wiki_id.map(|w| w.to_string()),
        wiki_name: e.wiki_name,
        path: e.path,
        ..Default::default()
    }
}
fn synthesis_wire(s: db::SynthesisRow) -> management::MemorySynthesis {
    management::MemorySynthesis {
        connection_id: s.connection_id.to_string(),
        model: s.model,
        instructions: s.instructions,
        ..Default::default()
    }
}
fn batch_wire(
    b: db::BatchRow,
    memories: &[db::BatchMemoryRow],
) -> management::MemorySynthesisBatch {
    use management::MemorySynthesisBatchStatus as S;
    management::MemorySynthesisBatch {
        id: b.id.to_string(),
        thread_id: b.thread_id.map(|t| t.to_string()),
        agent_id: b.agent_id.map(|a| a.to_string()),
        page_id: b.page_id.map(|p| p.to_string()),
        revision: b.revision.map(|r| r as u32),
        wiki_id: b.wiki_id.map(|w| w.to_string()),
        path: b.path,
        status: match b.status.as_str() {
            "applied" => S::Applied,
            "skipped" => S::Skipped,
            _ => S::Failed,
        }
        .into(),
        written: b.written as u32,
        superseded: b.superseded as u32,
        forgotten: b.forgotten as u32,
        dropped: b.dropped as u32,
        notes: b.notes,
        at: timestamp(b.at).into(),
        memories: memories
            .iter()
            .filter(|m| m.batch_id == b.id)
            .map(|m| management::MemorySynthesisBatchMemory {
                id: m.memory_id.to_string(),
                title: m.title.clone(),
                forgotten: m.forgotten,
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

impl MemoryService for Management {
    async fn list_memory_banks<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ListMemoryBanksRequest>,
    ) -> ServiceResult<impl Encodable<management::ListMemoryBanksResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let banks = self
            .0
            .banks(crate::rpc::search(r.search.as_deref())?)
            .await?;
        Response::ok(management::ListMemoryBanksResponse {
            banks: banks.into_iter().map(bank_wire).collect(),
            ..Default::default()
        })
    }
    async fn get_memory_bank<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::GetMemoryBankRequest>,
    ) -> ServiceResult<impl Encodable<management::GetMemoryBankResponse> + Send + use<'a>> {
        let bank = id(&request.to_owned_message().id)?;
        let bank = self.0.bank(bank).await?;
        Response::ok(management::GetMemoryBankResponse {
            bank: bank_wire(bank).into(),
            ..Default::default()
        })
    }
    async fn create_memory_bank<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::CreateMemoryBankRequest>,
    ) -> ServiceResult<impl Encodable<management::CreateMemoryBankResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let agent = r.agent_id.as_deref().map(id).transpose()?;
        let scope = scope_text(r.scope)?;
        let bank = self
            .0
            .create_bank(&r.name, &r.description, scope, agent)
            .await?;
        Response::ok(management::CreateMemoryBankResponse {
            bank: bank_wire(bank).into(),
            ..Default::default()
        })
    }
    async fn update_memory_bank<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::UpdateMemoryBankRequest>,
    ) -> ServiceResult<impl Encodable<management::UpdateMemoryBankResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let bank = id(&r.id)?;
        let bank = self
            .0
            .update_bank(bank, r.name.as_deref(), r.description.as_deref())
            .await?;
        Response::ok(management::UpdateMemoryBankResponse {
            bank: bank_wire(bank).into(),
            ..Default::default()
        })
    }
    async fn delete_memory_bank<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::DeleteMemoryBankRequest>,
    ) -> ServiceResult<impl Encodable<management::DeleteMemoryBankResponse> + Send + use<'a>> {
        let bank = id(&request.to_owned_message().id)?;
        self.0.delete_bank(bank).await?;
        Response::ok(management::DeleteMemoryBankResponse::default())
    }
    async fn list_agent_memory_banks<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ListAgentMemoryBanksRequest>,
    ) -> ServiceResult<impl Encodable<management::ListAgentMemoryBanksResponse> + Send + use<'a>>
    {
        let agent = id(&request.to_owned_message().agent_id)?;
        let links = self.0.agent_banks(agent).await?;
        Response::ok(management::ListAgentMemoryBanksResponse {
            banks: links.into_iter().map(link_wire).collect(),
            ..Default::default()
        })
    }
    async fn set_agent_memory_bank<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::SetAgentMemoryBankRequest>,
    ) -> ServiceResult<impl Encodable<management::SetAgentMemoryBankResponse> + Send + use<'a>>
    {
        let r = request.to_owned_message();
        let (agent, bank) = (id(&r.agent_id)?, id(&r.bank_id)?);
        let access = access_text(r.access)?;
        let link = self.0.set_link(agent, bank, access).await?;
        Response::ok(management::SetAgentMemoryBankResponse {
            bank: link_wire(link).into(),
            ..Default::default()
        })
    }
    async fn remove_agent_memory_bank<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::RemoveAgentMemoryBankRequest>,
    ) -> ServiceResult<impl Encodable<management::RemoveAgentMemoryBankResponse> + Send + use<'a>>
    {
        let r = request.to_owned_message();
        let (agent, bank) = (id(&r.agent_id)?, id(&r.bank_id)?);
        self.0.remove_link(agent, bank).await?;
        Response::ok(management::RemoveAgentMemoryBankResponse::default())
    }
    async fn list_memories<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ListMemoriesRequest>,
    ) -> ServiceResult<impl Encodable<management::ListMemoriesResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let bank = id(&r.bank_id)?;
        let filter = Filter {
            kind: kind_text(r.kind)?,
            path: r.path.as_deref().map(super::path).transpose()?,
            origin: origin_text(r.origin)?,
            identity: r.identity_id.as_deref().map(id).transpose()?,
            ..Default::default()
        };
        let limit = match r.limit {
            0 => 50,
            n => i64::from(n.min(200)),
        };
        if let Some(query) = r.query.as_deref().filter(|q| !q.trim().is_empty()) {
            let memories = self
                .0
                .recall(&[bank], Some(query), &filter, Person::Everyone, limit)
                .await?;
            return Response::ok(management::ListMemoriesResponse {
                memories: self.wire(&_ctx, memories).await?,
                ..Default::default()
            });
        }
        let after = match r.page_token.as_str() {
            "" => None,
            token => Some(id(token)?),
        };
        // One more than a page tells whether another follows.
        let mut memories = self
            .0
            .list(&[bank], &filter, Person::Everyone, after, limit + 1)
            .await?;
        let next_page_token = if memories.len() as i64 > limit {
            memories.truncate(limit as usize);
            memories
                .last()
                .map(|m| m.id.to_string())
                .unwrap_or_default()
        } else {
            String::new()
        };
        Response::ok(management::ListMemoriesResponse {
            memories: self
                .wire(&_ctx, memories.into_iter().map(|m| (m, vec![])).collect())
                .await?,
            next_page_token,
            ..Default::default()
        })
    }
    async fn list_memory_paths<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ListMemoryPathsRequest>,
    ) -> ServiceResult<impl Encodable<management::ListMemoryPathsResponse> + Send + use<'a>> {
        let bank = id(&request.to_owned_message().bank_id)?;
        Response::ok(management::ListMemoryPathsResponse {
            paths: self
                .0
                .paths(bank)
                .await?
                .into_iter()
                .map(|p| management::MemoryPath {
                    path: p.path,
                    count: p.count as u32,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        })
    }
    async fn list_memory_people<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ListMemoryPeopleRequest>,
    ) -> ServiceResult<impl Encodable<management::ListMemoryPeopleResponse> + Send + use<'a>> {
        let bank = id(&request.to_owned_message().bank_id)?;
        Response::ok(management::ListMemoryPeopleResponse {
            people: self
                .0
                .people(bank)
                .await?
                .into_iter()
                .map(|p| management::MemoryPerson {
                    identity_id: p.identity_id.to_string(),
                    name: p.name,
                    count: p.count as u32,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        })
    }
    async fn get_memory<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::GetMemoryRequest>,
    ) -> ServiceResult<impl Encodable<management::GetMemoryResponse> + Send + use<'a>> {
        let memory = self.0.memory(id(&request.to_owned_message().id)?).await?;
        let revisions = self.0.revisions(memory.id).await?;
        Response::ok(management::GetMemoryResponse {
            memory: self.wire(&_ctx, vec![(memory, vec![])]).await?.pop().into(),
            revisions: revisions
                .into_iter()
                .map(|r| management::MemoryRevision {
                    at: timestamp(r.at).into(),
                    operation: operation_wire(&r.operation).into(),
                    kind: kind_wire(&r.kind).into(),
                    path: r.path,
                    title: r.title,
                    content: r.content,
                    valid_from: r.valid_from.map(timestamp).into(),
                    valid_to: r.valid_to.map(timestamp).into(),
                    importance: r.importance,
                    origin: origin_wire(&r.origin).into(),
                    identity_id: r.identity_id.map(|i| i.to_string()),
                    agent_id: r.agent_id.map(|a| a.to_string()),
                    actor: actor_wire(&r.actor).into(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        })
    }
    async fn create_memory<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::CreateMemoryRequest>,
    ) -> ServiceResult<impl Encodable<management::CreateMemoryResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let bank = id(&r.bank_id)?;
        let identity = r.identity_id.as_deref().map(id).transpose()?;
        let created = self
            .0
            .create(
                Uuid::new_v4(),
                bank,
                Draft {
                    kind: kind_text(r.kind)?.unwrap_or_default(),
                    path: r.path,
                    title: r.title,
                    content: r.content,
                    valid_from: time(r.valid_from.into_option())?,
                    valid_to: time(r.valid_to.into_option())?,
                    ongoing: r.ongoing,
                    importance: r.importance.unwrap_or(0.5),
                },
                Source {
                    identity,
                    actor: Actor::Person,
                    thread: None,
                },
                None,
            )
            .await?;
        Response::ok(management::CreateMemoryResponse {
            memory: memory_wire(self.0.memory(created).await?).into(),
            ..Default::default()
        })
    }
    async fn update_memory<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::UpdateMemoryRequest>,
    ) -> ServiceResult<impl Encodable<management::UpdateMemoryResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let memory = id(&r.id)?;
        let updated = self
            .0
            .update(
                memory,
                Change {
                    kind: kind_text(r.kind)?,
                    path: r.path,
                    title: r.title,
                    content: r.content,
                    valid_from: time(r.valid_from.into_option())?,
                    valid_to: time(r.valid_to.into_option())?,
                    ongoing: r.ongoing,
                    clear_validity: r.clear_validity,
                    importance: r.importance,
                },
            )
            .await?;
        Response::ok(management::UpdateMemoryResponse {
            memory: memory_wire(updated).into(),
            ..Default::default()
        })
    }
    async fn delete_memory<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::DeleteMemoryRequest>,
    ) -> ServiceResult<impl Encodable<management::DeleteMemoryResponse> + Send + use<'a>> {
        let memory = id(&request.to_owned_message().id)?;
        self.0.forget(memory, None).await?;
        Response::ok(management::DeleteMemoryResponse::default())
    }
    async fn get_embedding_settings<'a>(
        &'a self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, management::GetEmbeddingSettingsRequest>,
    ) -> ServiceResult<impl Encodable<management::GetEmbeddingSettingsResponse> + Send + use<'a>>
    {
        Response::ok(management::GetEmbeddingSettingsResponse {
            settings: self.0.settings().await?.map(settings_wire).into(),
            ..Default::default()
        })
    }
    async fn set_embedding_settings<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::SetEmbeddingSettingsRequest>,
    ) -> ServiceResult<impl Encodable<management::SetEmbeddingSettingsResponse> + Send + use<'a>>
    {
        let s = request
            .to_owned_message()
            .settings
            .into_option()
            .ok_or_else(|| Error::Invalid("Settings are required".into()))?;
        let settings = self
            .0
            .set_settings(id(&s.connection_id)?, &s.model, s.dimensions as i32)
            .await?;
        Response::ok(management::SetEmbeddingSettingsResponse {
            settings: settings_wire(settings).into(),
            ..Default::default()
        })
    }
    async fn clear_embedding_settings<'a>(
        &'a self,
        _ctx: RequestContext,
        _request: ServiceRequest<'_, management::ClearEmbeddingSettingsRequest>,
    ) -> ServiceResult<impl Encodable<management::ClearEmbeddingSettingsResponse> + Send + use<'a>>
    {
        self.0.clear_settings().await?;
        Response::ok(management::ClearEmbeddingSettingsResponse::default())
    }
    async fn list_embedding_models<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ListEmbeddingModelsRequest>,
    ) -> ServiceResult<impl Encodable<management::ListEmbeddingModelsResponse> + Send + use<'a>>
    {
        let connection = id(&request.to_owned_message().connection_id)?;
        Response::ok(management::ListEmbeddingModelsResponse {
            models: self
                .0
                .models(connection)
                .await?
                .into_iter()
                .map(|(model, dimensions)| management::EmbeddingModel {
                    model,
                    dimensions,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        })
    }
    async fn get_memory_synthesis<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::GetMemorySynthesisRequest>,
    ) -> ServiceResult<impl Encodable<management::GetMemorySynthesisResponse> + Send + use<'a>>
    {
        let bank = id(&request.to_owned_message().bank_id)?;
        self.0.bank(bank).await?;
        let synthesis = self.0.synthesis(bank).await?;
        let retaining = self.0.retaining(bank).await?;
        let wikis = self.0.bank_wikis(bank).await?;
        let pending = self.0.synthesis_pending(bank).await?;
        let count = |pages: bool| -> u32 {
            pending
                .iter()
                .filter(|p| p.wiki_id.is_some() == pages)
                .map(|p| p.count as u32)
                .sum()
        };
        Response::ok(management::GetMemorySynthesisResponse {
            failures: synthesis.as_ref().map_or(0, |s| s.attempts as u32),
            last_error: synthesis
                .as_ref()
                .map(|s| s.last_error.clone())
                .unwrap_or_default(),
            last_run_at: synthesis
                .as_ref()
                .and_then(|s| s.last_run_at)
                .map(timestamp)
                .into(),
            pending_conversations: count(false),
            pending_pages: count(true),
            synthesis: synthesis.map(synthesis_wire).into(),
            retaining_agent_ids: retaining.iter().map(Uuid::to_string).collect(),
            wikis: wikis
                .into_iter()
                .map(|w| management::MemoryBankWiki {
                    wiki_id: w.wiki_id.to_string(),
                    name: w.name,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        })
    }
    async fn set_memory_synthesis<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::SetMemorySynthesisRequest>,
    ) -> ServiceResult<impl Encodable<management::SetMemorySynthesisResponse> + Send + use<'a>>
    {
        let r = request.to_owned_message();
        let bank = id(&r.bank_id)?;
        let synthesis = r.synthesis.into_option().unwrap_or_default();
        let connection = id(&synthesis.connection_id)?;
        let saved = self
            .0
            .set_synthesis(bank, connection, &synthesis.model, &synthesis.instructions)
            .await?;
        Response::ok(management::SetMemorySynthesisResponse {
            synthesis: synthesis_wire(saved).into(),
            ..Default::default()
        })
    }
    async fn clear_memory_synthesis<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ClearMemorySynthesisRequest>,
    ) -> ServiceResult<impl Encodable<management::ClearMemorySynthesisResponse> + Send + use<'a>>
    {
        let bank = id(&request.to_owned_message().bank_id)?;
        self.0.clear_synthesis(bank).await?;
        Response::ok(management::ClearMemorySynthesisResponse::default())
    }
    async fn set_memory_bank_retain<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::SetMemoryBankRetainRequest>,
    ) -> ServiceResult<impl Encodable<management::SetMemoryBankRetainResponse> + Send + use<'a>>
    {
        let r = request.to_owned_message();
        let (bank, agent) = (id(&r.bank_id)?, id(&r.agent_id)?);
        self.0.set_retain(agent, bank, r.auto_retain).await?;
        Response::ok(management::SetMemoryBankRetainResponse::default())
    }
    async fn set_memory_bank_wiki<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::SetMemoryBankWikiRequest>,
    ) -> ServiceResult<impl Encodable<management::SetMemoryBankWikiResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let (bank, wiki) = (id(&r.bank_id)?, id(&r.wiki_id)?);
        self.0.set_bank_wiki(bank, wiki).await?;
        Response::ok(management::SetMemoryBankWikiResponse::default())
    }
    async fn remove_memory_bank_wiki<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::RemoveMemoryBankWikiRequest>,
    ) -> ServiceResult<impl Encodable<management::RemoveMemoryBankWikiResponse> + Send + use<'a>>
    {
        let r = request.to_owned_message();
        let (bank, wiki) = (id(&r.bank_id)?, id(&r.wiki_id)?);
        self.0.remove_bank_wiki(bank, wiki).await?;
        Response::ok(management::RemoveMemoryBankWikiResponse::default())
    }
    async fn list_memory_synthesis_batches<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ListMemorySynthesisBatchesRequest>,
    ) -> ServiceResult<
        impl Encodable<management::ListMemorySynthesisBatchesResponse> + Send + use<'a>,
    > {
        let bank = id(&request.to_owned_message().bank_id)?;
        let (batches, memories) = self.0.batches(bank).await?;
        Response::ok(management::ListMemorySynthesisBatchesResponse {
            batches: batches
                .into_iter()
                .map(|b| batch_wire(b, &memories))
                .collect(),
            ..Default::default()
        })
    }
    async fn run_memory_synthesis<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::RunMemorySynthesisRequest>,
    ) -> ServiceResult<impl Encodable<management::RunMemorySynthesisResponse> + Send + use<'a>>
    {
        let bank = id(&request.to_owned_message().bank_id)?;
        self.0.run_synthesis_now(bank).await?;
        Response::ok(management::RunMemorySynthesisResponse::default())
    }
    async fn list_chat_models<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ListChatModelsRequest>,
    ) -> ServiceResult<impl Encodable<management::ListChatModelsResponse> + Send + use<'a>> {
        let connection = id(&request.to_owned_message().connection_id)?;
        Response::ok(management::ListChatModelsResponse {
            models: self.0.chat_models(connection).await?,
            ..Default::default()
        })
    }
}
