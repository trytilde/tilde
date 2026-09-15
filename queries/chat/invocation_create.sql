INSERT INTO chat_invocations(id,run_id,thread_id,agent_id,status,traceparent,tracestate,deployment_id,history_through_message_id)
VALUES($1,$2,$3,$4,'pending',$5,$6,$7,(SELECT id FROM chat_messages WHERE thread_id=$3 ORDER BY created_at DESC,id DESC LIMIT 1));
