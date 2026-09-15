INSERT INTO chat_inputs(invocation_id,origin_invocation_id,id,text,history_through_message_id)
SELECT $1,$1,$2,$3,COALESCE(
 (SELECT m.id FROM chat_messages m WHERE m.id=$2 AND m.thread_id=v.thread_id),
 (SELECT m.id FROM chat_messages m WHERE m.thread_id=v.thread_id ORDER BY m.created_at DESC,m.id DESC LIMIT 1))
FROM chat_invocations v WHERE v.id=$1
ON CONFLICT(origin_invocation_id,id) DO NOTHING;
