--! run (p1, p2, p3, p4, p5, p6, p7?)
INSERT INTO chat_invocations(id,run_id,thread_id,agent_id,status,traceparent,tracestate,deployment_id,history_through_message_id)
VALUES(:p1,:p2,:p3,:p4,'pending',:p5,:p6,:p7,(SELECT id FROM chat_messages WHERE thread_id=:p3 ORDER BY created_at DESC,id DESC LIMIT 1));
