--! run (p1, p2, p3, p4, p5, p6)
INSERT INTO chat_channel_access_audit(connection_id,event_id,agent_id,identity_id,access_mode,accepted) VALUES(:p1,:p2,:p3,:p4,:p5,:p6) ON CONFLICT DO NOTHING;
