--: Record(account_label?, token_expires_at?)

-- Connections carry no roles of their own; attached agents are listed only when the caller can view them.
--! run (p1?, p2?, p3, p4?, p5?, is_admin, caller_user?, caller_key?, caller_groups) : Record
SELECT c.id,c.name,c.provider_id,c.type_id,c.status,c.account_label,c.token_expires_at,c.credential_version,c.created_at,c.updated_at,t.channel_capable,t.inference_capable,
 COALESCE((SELECT jsonb_agg(jsonb_build_object('capability',ca.capability,'id',a.id,'name',a.name,'alias',ca.alias) ORDER BY ca.capability,a.id)
 FROM connection_agents ca JOIN agents a ON a.id=ca.agent_id WHERE ca.connection_id=c.id AND iam_held('agent',a.id,ARRAY['view']::TEXT[],:is_admin,:caller_user,:caller_key,:caller_groups::TEXT[])),'[]'::jsonb) AS associated_agents
 FROM connections c JOIN connection_types t ON(t.provider_id=c.provider_id AND t.type_id=c.type_id) WHERE (:p1::TIMESTAMPTZ IS NULL OR (c.created_at,c.id)<(:p1,:p2))
 AND (:p4::UUID IS NULL OR EXISTS(SELECT 1 FROM connection_agents ca WHERE ca.connection_id=c.id AND ca.agent_id=:p4 AND (:p5::TEXT IS NULL OR ca.capability=:p5)))
 AND (:p4::UUID IS NOT NULL OR :p5::TEXT IS NULL
  OR (:p5='channel' AND t.channel_capable AND NOT EXISTS(SELECT 1 FROM connection_agents ca WHERE ca.connection_id=c.id AND ca.capability='channel'))
  OR (:p5='inference' AND t.inference_capable))
 ORDER BY c.created_at DESC,c.id DESC LIMIT :p3;
