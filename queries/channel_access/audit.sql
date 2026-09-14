INSERT INTO chat_channel_access_audit(connection_id,event_id,agent_id,identity_id,access_mode,accepted) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING;
