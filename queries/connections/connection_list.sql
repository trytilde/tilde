--: Record(account_label?, token_expires_at?)

-- Every connection offers the skills capability; its type decides chat, inference and tools.
-- p6 searches the name, account label and provider name; p7 and p8 match provider and status;
-- p9 the provider's provider_source.
--! run (p1?, p2?, p3, p4?, p5?, p6?, p7?, p8?, p9?) : Record
SELECT c.id,c.name,c.provider_id,c.type_id,c.status,c.account_label,c.token_expires_at,c.credential_version,c.created_at,c.updated_at,t.channel_capable,t.inference_capable,t.tool_capable,
 COALESCE((SELECT jsonb_agg(jsonb_build_object('capability',ca.capability,'id',a.id,'name',a.name,'alias',ca.alias) ORDER BY ca.capability,a.id)
 FROM connection_agents ca JOIN agents a ON a.id=ca.agent_id WHERE ca.connection_id=c.id
 ),'[]'::jsonb) AS associated_agents
 FROM connections c JOIN connection_types t ON(t.provider_id=c.provider_id AND t.type_id=c.type_id) WHERE (:p1::TIMESTAMPTZ IS NULL OR (c.created_at,c.id)<(:p1,:p2))
 AND (:p4::UUID IS NULL OR EXISTS(SELECT 1 FROM connection_agents ca WHERE ca.connection_id=c.id AND ca.agent_id=:p4 AND (:p5::TEXT IS NULL OR ca.capability=:p5)))
 AND (:p4::UUID IS NOT NULL OR :p5::TEXT IS NULL
  OR (:p5='channel' AND t.channel_capable AND NOT EXISTS(SELECT 1 FROM connection_agents ca WHERE ca.connection_id=c.id AND ca.capability='channel'))
  OR (:p5='inference' AND t.inference_capable)
  OR :p5='skills'
  OR (:p5='tool' AND t.tool_capable))
 AND (:p6::TEXT IS NULL OR c.name ILIKE '%'||:p6||'%' OR c.account_label ILIKE '%'||:p6||'%'
  OR EXISTS(SELECT 1 FROM connection_providers p WHERE p.provider_id=c.provider_id AND p.name ILIKE '%'||:p6||'%'))
 AND (:p7::TEXT IS NULL OR c.provider_id=:p7)
 AND (:p8::TEXT IS NULL OR c.status=:p8)
 AND (:p9::TEXT IS NULL OR provider_source(c.provider_id)=:p9)
 ORDER BY c.created_at DESC,c.id DESC LIMIT :p3;
