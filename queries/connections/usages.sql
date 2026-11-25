--: Record(id?, resource_kind?, resource_id?)

-- Everything that uses each of `ids`, by the names its users know: a connection in use is never
-- disconnected or deleted. `kind`: `agent` (an assignment; `detail` its capability),
-- `personal_agent` (an agent a personal connection allows), `tool_source` (an agent's;
-- `detail` its slug), `sandbox_tool_source` (a sandbox blueprint's; `detail` its slug),
-- `routine` (fired by the connection's signals; `detail` its agent and signal), `sandbox_blueprint`, `sandbox` (a live sandbox on it its blueprint no longer names;
-- `detail` its status), `sandbox_egress` (a blueprint's egress grant; `detail` its slug),
-- `memory` (the memory embeddings; `detail` the model), `memory_synthesis` (a bank's; `detail`
-- the model).
-- Not uses: what the connection owns (values, tools, skills, grants), history (chat, audit,
-- inference requests), queues (ingress outbox, tool host calls) and an assignment's inference
-- budget (a spend policy; the connection row is never deleted by a disconnect).
-- `resource_kind`/`resource_id` name the resource whose roles decide who may see the use (none:
-- everyone).
--! run (ids) : Record
SELECT u.connection_id,u.kind,u.id,u.name,u.detail,u.resource_kind,u.resource_id
FROM (
 SELECT ca.connection_id,'agent' AS kind,a.id,a.name,ca.capability AS detail,'agent' AS resource_kind,a.id AS resource_id
 FROM connection_agents ca JOIN agents a ON a.id=ca.agent_id WHERE ca.connection_id=ANY(:ids::UUID[])
 UNION ALL
 SELECT p.connection_id,'personal_agent',a.id,a.name,'','agent',a.id
 FROM personal_connection_agents p JOIN agents a ON a.id=p.agent_id WHERE p.connection_id=ANY(:ids::UUID[])
 UNION ALL
 SELECT s.connection_id,'tool_source',a.id,a.name,s.slug,'agent',a.id
 FROM agent_tool_sources s JOIN agents a ON a.id=s.agent_id WHERE s.connection_id=ANY(:ids::UUID[])
 UNION ALL
 SELECT s.connection_id,'sandbox_tool_source',b.id,b.name,s.slug,'sandbox_blueprint',b.id
 FROM agent_tool_sources s JOIN sandbox_blueprints b ON b.id=s.sandbox_blueprint_id WHERE s.connection_id=ANY(:ids::UUID[])
 UNION ALL
 SELECT r.connection_id,'routine',r.id,r.name,a.name||', '||r.signal_type,'agent',a.id
 FROM routines r JOIN agents a ON a.id=r.agent_id WHERE r.connection_id=ANY(:ids::UUID[])
 UNION ALL
 SELECT b.connection_id,'sandbox_blueprint',b.id,b.name,'','sandbox_blueprint',b.id
 FROM sandbox_blueprints b WHERE b.connection_id=ANY(:ids::UUID[])
 UNION ALL
 -- A live sandbox left on a connection its blueprint no longer names (otherwise the blueprint is
 -- the use): it sleeps, wakes and terminates through it.
 SELECT s.connection_id,'sandbox',s.id,b.name,s.status,'sandbox_blueprint',b.id
 FROM sandboxes s JOIN sandbox_blueprints b ON b.id=s.blueprint_id
 WHERE s.connection_id=ANY(:ids::UUID[]) AND s.status<>'failed' AND b.connection_id<>s.connection_id
 UNION ALL
 SELECT g.connection_id,'sandbox_egress',b.id,b.name,g.slug,'sandbox_blueprint',b.id
 FROM sandbox_blueprint_egress g JOIN sandbox_blueprints b ON b.id=g.blueprint_id WHERE g.connection_id=ANY(:ids::UUID[])
 UNION ALL
 SELECT m.connection_id,'memory',NULL,'Memory embeddings',m.model,NULL,NULL
 FROM memory_settings m WHERE m.connection_id=ANY(:ids::UUID[])
 UNION ALL
 SELECT s.connection_id,'memory_synthesis',k.id,k.name,s.model,'memory_bank',k.id
 FROM memory_synthesis s JOIN memory_banks k ON k.id=s.bank_id WHERE s.connection_id=ANY(:ids::UUID[])
) u
ORDER BY u.connection_id,u.kind,u.name,u.detail;
