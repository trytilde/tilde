--! run (p1, p2, p3, p4, p5, p6?, p7?, p8, p9)
INSERT INTO chat_messages(id,thread_id,participant_id,text,status,invocation_id,in_reply_to_message_id,stream_expires_at,traceparent,tracestate) VALUES(:p1,:p2,:p3,:p4,:p5,:p6,:p7,CASE WHEN :p5='streaming' THEN NOW()+INTERVAL '30 seconds' ELSE NULL END,:p8,:p9);
