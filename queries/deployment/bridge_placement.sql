INSERT INTO sidecar_conversations(thread_id,agent_id,storage) VALUES($1,$2,'corrosion') ON CONFLICT DO NOTHING;
