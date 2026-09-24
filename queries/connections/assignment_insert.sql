--! run (p1, p2, p3, p4?)
INSERT INTO connection_agents(connection_id,capability,agent_id,access_mode,alias) SELECT :p1,:p2,:p3,CASE WHEN provider_id='github' THEN 'disabled' ELSE 'private' END,:p4 FROM connections WHERE id=:p1 ON CONFLICT (connection_id,capability,agent_id) DO UPDATE SET alias=excluded.alias;
