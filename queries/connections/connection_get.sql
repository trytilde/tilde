SELECT c.id,c.name,c.provider_id,c.type_id,c.status,c.account_label,c.token_expires_at,c.credential_version,c.created_at,c.updated_at,t.channel_capable,
 COALESCE((SELECT jsonb_agg(jsonb_build_object('capability',ca.capability,'id',a.id,'name',a.name,'endpoint_url',a.endpoint_url) ORDER BY ca.capability,a.id)
 FROM connection_agents ca JOIN agents a ON a.id=ca.agent_id WHERE ca.connection_id=c.id),'[]'::jsonb) AS "associated_agents!: sqlx::types::Json<Vec<AssociatedAgent>>"
 FROM connections c JOIN connection_types t ON(t.provider_id=c.provider_id AND t.type_id=c.type_id) WHERE c.id=$1;
