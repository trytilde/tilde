//! The wiki management service.
use super::{GREP_LINES, SEARCH_RESULTS, Wikis, db};
use crate::chat::audit::timestamp;
use crate::error::Error;
use crate::proto::tilde::management::v1 as management;
use crate::services::tilde::management::v1::WikiService;
use connectrpc::{Encodable, RequestContext, Response, ServiceRequest, ServiceResult};
use std::sync::Arc;
use uuid::Uuid;

pub fn router(wikis: Wikis) -> axum::Router {
    crate::rpc::mount(
        connectrpc::Router::new().add_service(Arc::new(Management(wikis))),
        1024 * 1024,
    )
}
struct Management(Wikis);

fn id(value: &str) -> Result<Uuid, Error> {
    Uuid::parse_str(value).map_err(|_| Error::Invalid("Invalid UUID".into()))
}
fn access_text(access: buffa::EnumValue<management::WikiAccess>) -> Result<&'static str, Error> {
    match access.as_known() {
        Some(management::WikiAccess::Read) => Ok("read"),
        Some(management::WikiAccess::ReadWrite) => Ok("read_write"),
        _ => Err(Error::Invalid("Access is READ or READ_WRITE".into())),
    }
}
fn access_wire(access: &str) -> management::WikiAccess {
    match access {
        "read_write" => management::WikiAccess::ReadWrite,
        _ => management::WikiAccess::Read,
    }
}
fn operation_wire(operation: &str) -> management::WikiOperation {
    use management::WikiOperation as O;
    match operation {
        "create" => O::Create,
        "update" => O::Update,
        "move" => O::Move,
        "delete" => O::Delete,
        _ => O::Unspecified,
    }
}
fn revision(value: u32) -> Result<i32, Error> {
    i32::try_from(value).map_err(|_| Error::Invalid("Invalid revision".into()))
}
fn wiki_wire(w: db::WikiRow) -> management::Wiki {
    management::Wiki {
        id: w.id.to_string(),
        name: w.name,
        description: w.description,
        agent_id: w.agent_id.map(|a| a.to_string()),
        links: w
            .agent_ids
            .iter()
            .zip(&w.link_access)
            .map(|(agent, access)| management::WikiLink {
                agent_id: agent.to_string(),
                access: access_wire(access).into(),
                ..Default::default()
            })
            .collect(),
        page_count: w.page_count as u32,
        created_at: timestamp(w.created_at).into(),
        ..Default::default()
    }
}
fn link_wire(l: db::AgentWikiRow) -> management::AgentWiki {
    management::AgentWiki {
        agent_id: l.agent_id.to_string(),
        wiki_id: l.wiki_id.to_string(),
        wiki_name: l.wiki_name,
        access: access_wire(&l.access).into(),
        own: l.own,
        ..Default::default()
    }
}
fn page_wire(p: db::PageRow) -> management::WikiPage {
    management::WikiPage {
        id: p.id.to_string(),
        wiki_id: p.wiki_id.to_string(),
        path: p.path,
        title: p.title,
        content: p.content,
        revision: p.revision as u32,
        created_at: timestamp(p.created_at).into(),
        updated_at: timestamp(p.updated_at).into(),
        ..Default::default()
    }
}
fn summary_wire(
    id: Uuid,
    path: String,
    title: String,
    revision: i32,
    updated_at: chrono::DateTime<chrono::Utc>,
) -> management::WikiPageSummary {
    management::WikiPageSummary {
        id: id.to_string(),
        path,
        title,
        revision: revision as u32,
        updated_at: timestamp(updated_at).into(),
        ..Default::default()
    }
}
fn revision_wire(r: db::RevisionRow) -> management::WikiPageRevision {
    management::WikiPageRevision {
        revision: r.revision as u32,
        operation: operation_wire(&r.operation).into(),
        path: r.path,
        title: r.title,
        agent_id: r.agent_id.map(|a| a.to_string()),
        thread_id: r.thread_id.map(|t| t.to_string()),
        at: timestamp(r.at).into(),
        ..Default::default()
    }
}
/// A person's write through management, which no retry repeats.
fn person() -> super::Write {
    super::Write {
        call: Uuid::new_v4(),
        agent: None,
        thread: None,
    }
}

impl WikiService for Management {
    async fn list_wikis<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ListWikisRequest>,
    ) -> ServiceResult<impl Encodable<management::ListWikisResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let wikis = self
            .0
            .wikis(crate::rpc::search(r.search.as_deref())?)
            .await?;
        Response::ok(management::ListWikisResponse {
            wikis: wikis.into_iter().map(wiki_wire).collect(),
            ..Default::default()
        })
    }
    async fn get_wiki<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::GetWikiRequest>,
    ) -> ServiceResult<impl Encodable<management::GetWikiResponse> + Send + use<'a>> {
        let wiki = id(&request.to_owned_message().id)?;
        let wiki = self.0.wiki(wiki).await?;
        Response::ok(management::GetWikiResponse {
            wiki: wiki_wire(wiki).into(),
            ..Default::default()
        })
    }
    async fn create_wiki<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::CreateWikiRequest>,
    ) -> ServiceResult<impl Encodable<management::CreateWikiResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let agent = r.agent_id.as_deref().map(id).transpose()?;
        let wiki = self.0.create_wiki(&r.name, &r.description, agent).await?;
        Response::ok(management::CreateWikiResponse {
            wiki: wiki_wire(wiki).into(),
            ..Default::default()
        })
    }
    async fn update_wiki<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::UpdateWikiRequest>,
    ) -> ServiceResult<impl Encodable<management::UpdateWikiResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let wiki = id(&r.id)?;
        let wiki = self
            .0
            .update_wiki(wiki, r.name.as_deref(), r.description.as_deref())
            .await?;
        Response::ok(management::UpdateWikiResponse {
            wiki: wiki_wire(wiki).into(),
            ..Default::default()
        })
    }
    async fn delete_wiki<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::DeleteWikiRequest>,
    ) -> ServiceResult<impl Encodable<management::DeleteWikiResponse> + Send + use<'a>> {
        let wiki = id(&request.to_owned_message().id)?;
        self.0.delete_wiki(wiki).await?;
        Response::ok(management::DeleteWikiResponse::default())
    }
    async fn list_agent_wikis<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ListAgentWikisRequest>,
    ) -> ServiceResult<impl Encodable<management::ListAgentWikisResponse> + Send + use<'a>> {
        let agent = id(&request.to_owned_message().agent_id)?;
        let links = self.0.agent_wikis(agent).await?;
        Response::ok(management::ListAgentWikisResponse {
            wikis: links.into_iter().map(link_wire).collect(),
            ..Default::default()
        })
    }
    async fn set_agent_wiki<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::SetAgentWikiRequest>,
    ) -> ServiceResult<impl Encodable<management::SetAgentWikiResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let (agent, wiki) = (id(&r.agent_id)?, id(&r.wiki_id)?);
        let access = access_text(r.access)?;
        let link = self.0.set_link(agent, wiki, access).await?;
        Response::ok(management::SetAgentWikiResponse {
            wiki: link_wire(link).into(),
            ..Default::default()
        })
    }
    async fn remove_agent_wiki<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::RemoveAgentWikiRequest>,
    ) -> ServiceResult<impl Encodable<management::RemoveAgentWikiResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let (agent, wiki) = (id(&r.agent_id)?, id(&r.wiki_id)?);
        self.0.remove_link(agent, wiki).await?;
        Response::ok(management::RemoveAgentWikiResponse::default())
    }
    async fn list_wiki_pages<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::ListWikiPagesRequest>,
    ) -> ServiceResult<impl Encodable<management::ListWikiPagesResponse> + Send + use<'a>> {
        let wiki = id(&request.to_owned_message().wiki_id)?;
        let (pages, more) = self.0.pages(wiki, None, super::LIST_PAGES).await?;
        Response::ok(management::ListWikiPagesResponse {
            pages: pages
                .into_iter()
                .map(|p| summary_wire(p.id, p.path, p.title, p.revision, p.updated_at))
                .collect(),
            more,
            ..Default::default()
        })
    }
    async fn get_wiki_page<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::GetWikiPageRequest>,
    ) -> ServiceResult<impl Encodable<management::GetWikiPageResponse> + Send + use<'a>> {
        let page = self.0.page(id(&request.to_owned_message().id)?).await?;
        let revisions = self.0.revisions(page.id).await?;
        Response::ok(management::GetWikiPageResponse {
            page: page_wire(page).into(),
            revisions: revisions.into_iter().map(revision_wire).collect(),
            ..Default::default()
        })
    }
    async fn get_wiki_page_revision<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::GetWikiPageRevisionRequest>,
    ) -> ServiceResult<impl Encodable<management::GetWikiPageRevisionResponse> + Send + use<'a>>
    {
        let r = request.to_owned_message();
        let found = self
            .0
            .revision(id(&r.page_id)?, revision(r.revision)?)
            .await?;
        Response::ok(management::GetWikiPageRevisionResponse {
            revision: revision_wire(db::RevisionRow {
                revision: found.revision,
                operation: found.operation,
                path: found.path,
                title: found.title,
                agent_id: found.agent_id,
                thread_id: found.thread_id,
                at: found.at,
            })
            .into(),
            content: found.content,
            ..Default::default()
        })
    }
    async fn search_wiki_pages<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::SearchWikiPagesRequest>,
    ) -> ServiceResult<impl Encodable<management::SearchWikiPagesResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let wiki = id(&r.wiki_id)?;
        let found = self.0.search(&[wiki], &r.query, SEARCH_RESULTS).await?;
        Response::ok(management::SearchWikiPagesResponse {
            results: found
                .into_iter()
                .map(|p| management::WikiSearchResult {
                    page: summary_wire(p.id, p.path, p.title, p.revision, p.updated_at).into(),
                    excerpt: p.excerpt,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        })
    }
    async fn grep_wiki_pages<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::GrepWikiPagesRequest>,
    ) -> ServiceResult<impl Encodable<management::GrepWikiPagesResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let wiki = id(&r.wiki_id)?;
        let (lines, more) = self
            .0
            .grep(&[wiki], &r.pattern, r.regex, r.ignore_case, GREP_LINES)
            .await?;
        Response::ok(management::GrepWikiPagesResponse {
            lines: lines
                .into_iter()
                .map(|l| management::WikiGrepLine {
                    page_path: l.path,
                    line_number: l.line_number as u32,
                    line: l.line,
                    truncated: l.truncated,
                    ..Default::default()
                })
                .collect(),
            more,
            ..Default::default()
        })
    }
    async fn create_wiki_page<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::CreateWikiPageRequest>,
    ) -> ServiceResult<impl Encodable<management::CreateWikiPageResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let wiki = id(&r.wiki_id)?;
        let page = self
            .0
            .create_page(&person(), wiki, &r.path, &r.title, &r.content)
            .await?;
        Response::ok(management::CreateWikiPageResponse {
            page: page_wire(page).into(),
            ..Default::default()
        })
    }
    async fn update_wiki_page<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::UpdateWikiPageRequest>,
    ) -> ServiceResult<impl Encodable<management::UpdateWikiPageResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let page = self.0.page(id(&r.id)?).await?;
        let page = self
            .0
            .update_page(
                &person(),
                page.id,
                revision(r.expected_revision)?,
                r.title.as_deref(),
                r.content.map(super::Body::Replace),
            )
            .await?;
        Response::ok(management::UpdateWikiPageResponse {
            page: page_wire(page).into(),
            ..Default::default()
        })
    }
    async fn move_wiki_page<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::MoveWikiPageRequest>,
    ) -> ServiceResult<impl Encodable<management::MoveWikiPageResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let page = self.0.page(id(&r.id)?).await?;
        let page = self
            .0
            .move_page(&person(), page.id, revision(r.expected_revision)?, &r.path)
            .await?;
        Response::ok(management::MoveWikiPageResponse {
            page: page_wire(page).into(),
            ..Default::default()
        })
    }
    async fn delete_wiki_page<'a>(
        &'a self,
        _ctx: RequestContext,
        request: ServiceRequest<'_, management::DeleteWikiPageRequest>,
    ) -> ServiceResult<impl Encodable<management::DeleteWikiPageResponse> + Send + use<'a>> {
        let r = request.to_owned_message();
        let page = self.0.page(id(&r.id)?).await?;
        self.0
            .delete_page(&person(), page.id, revision(r.expected_revision)?)
            .await?;
        Response::ok(management::DeleteWikiPageResponse::default())
    }
}
