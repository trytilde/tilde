--! run (p1, p2, p3)
INSERT INTO agent_channel_identities(connection_id,agent_id,identity_type,value)
SELECT connection_id,agent_id,:p2,:p3 FROM connection_agents WHERE connection_id=:p1 AND capability='channel'
ON CONFLICT(connection_id,agent_id) DO UPDATE SET identity_type=EXCLUDED.identity_type,value=EXCLUDED.value;
