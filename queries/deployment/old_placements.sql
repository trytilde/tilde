INSERT INTO sidecar_conversations(thread_id,agent_id,storage,retired_at) SELECT thread_id,$1,'postgres',NOW() FROM chat_participants WHERE agent_id=$1 ON CONFLICT DO NOTHING;
