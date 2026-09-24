--! run (p1, p2, p3, p4, p5, p6?, p7, p8, p9?)
INSERT INTO chat_messages(id,thread_id,participant_id,text,status,in_reply_to_message_id,created_at,format,subject) VALUES(:p1,:p2,:p3,:p4,:p5,:p6,:p7,:p8,:p9) ON CONFLICT(id) DO UPDATE SET text=EXCLUDED.text,status=EXCLUDED.status,format=EXCLUDED.format,subject=EXCLUDED.subject;
