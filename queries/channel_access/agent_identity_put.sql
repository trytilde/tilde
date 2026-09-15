INSERT INTO agent_channel_identities(connection_id,agent_id,identity_type,value)
SELECT connection_id,agent_id,$2,$3 FROM connection_agents WHERE connection_id=$1 AND capability='channel'
ON CONFLICT(connection_id,agent_id) DO UPDATE SET identity_type=EXCLUDED.identity_type,value=EXCLUDED.value;
