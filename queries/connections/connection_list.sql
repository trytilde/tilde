SELECT c.id,c.name,c.provider_id,c.type_id,c.status,c.account_label,c.token_expires_at,c.credential_version,c.created_at,c.updated_at,t.channel_capable,
 COALESCE((SELECT jsonb_agg(jsonb_build_object('capability',ca.capability,'id',a.id,'name',a.name,'endpoint_url',a.endpoint_url) ORDER BY ca.capability,a.id)
 FROM connection_agents ca JOIN agents a ON a.id=ca.agent_id WHERE ca.connection_id=c.id),'[]'::jsonb) AS "associated_agents!: sqlx::types::Json<Vec<AssociatedAgent>>"
 FROM connections c JOIN connection_types t ON(t.provider_id=c.provider_id AND t.type_id=c.type_id) WHERE ($1::TIMESTAMPTZ IS NULL OR (c.created_at,c.id)<($1,$2))
 AND ($4::UUID IS NULL OR EXISTS(SELECT 1 FROM connection_agents ca WHERE ca.connection_id=c.id AND ca.agent_id=$4))
 AND (NOT $5::BOOLEAN OR (t.channel_capable AND NOT EXISTS(SELECT 1 FROM connection_agents ca WHERE ca.connection_id=c.id AND ca.capability='channel')))
 ORDER BY c.created_at DESC,c.id DESC LIMIT $3;
