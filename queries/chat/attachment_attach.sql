--! run (p1, p2, p3)
INSERT INTO chat_message_attachments(thread_id,message_id,attachment_id) VALUES(:p1,:p2,:p3);
