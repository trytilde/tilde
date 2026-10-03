--: Record(account_label?, token_expires_at?)

--! run (p1) : Record
SELECT c.id,c.name,c.provider_id,c.type_id,c.status,c.account_label,c.token_expires_at,c.credential_version,c.created_at,c.updated_at,t.channel_capable,t.inference_capable,t.tool_capable,t.signal_capable,
 COALESCE((SELECT jsonb_agg(jsonb_build_object('capability',ca.capability,'id',a.id,'name',a.name,'alias',ca.alias) ORDER BY ca.capability,a.id)
 FROM connection_agents ca JOIN agents a ON a.id=ca.agent_id WHERE ca.connection_id=c.id),'[]'::jsonb) AS associated_agents
 FROM connections c JOIN connection_types t ON(t.provider_id=c.provider_id AND t.type_id=c.type_id) WHERE c.id=:p1;
