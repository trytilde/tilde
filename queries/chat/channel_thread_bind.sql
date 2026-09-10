INSERT INTO chat_channel_threads(thread_id,connection_id,external_id,agent_id) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING;
