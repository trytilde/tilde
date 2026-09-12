INSERT INTO sidecar_bridge_receipts(thread_id,sequence,agent_id) VALUES($1,$2,$3) ON CONFLICT DO NOTHING RETURNING sequence;
