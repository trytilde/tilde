--! run (p1, p2, p3, p4)
INSERT INTO chat_channel_identity_access(connection_id,agent_id,identity_id,allowed) VALUES(:p1,:p2,:p3,:p4)
ON CONFLICT(connection_id,agent_id,identity_id) DO UPDATE SET allowed=excluded.allowed;
