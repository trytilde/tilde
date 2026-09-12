INSERT INTO sidecar_outbox(id,agent_id,kind,payload) SELECT $1,$2,'attachment',$3 WHERE EXISTS(SELECT 1 FROM chat_attachments WHERE id=$4 AND object_key IS NULL) ON CONFLICT DO NOTHING;
