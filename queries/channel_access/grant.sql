INSERT INTO chat_channel_identity_access(connection_id,agent_id,identity_id,allowed) VALUES($1,$2,$3,$4)
ON CONFLICT(connection_id,agent_id,identity_id) DO UPDATE SET allowed=excluded.allowed;
