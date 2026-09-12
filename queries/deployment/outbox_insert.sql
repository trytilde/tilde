INSERT INTO sidecar_outbox(id,agent_id,kind,payload) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING;
