INSERT INTO chat_message_dispatch(message_id,agent_id)
SELECT m.id,$2 FROM chat_messages m WHERE m.thread_id=$1 AND m.id=$3 AND
(EXISTS(SELECT 1 FROM chat_runs r WHERE r.thread_id=$1 AND r.agent_id=$2 AND r.idempotency_key=m.id::text)
 OR EXISTS(SELECT 1 FROM sidecar_commands c WHERE c.thread_id=$1 AND c.agent_id=$2 AND c.input_id=m.id AND c.kind='steer'))
ON CONFLICT DO NOTHING;
