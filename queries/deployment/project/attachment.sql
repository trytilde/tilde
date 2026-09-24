--! run (p1, p2, p3, p4, p5, p6)
INSERT INTO chat_attachments(id,thread_id,filename,media_type,size_bytes,sha256,content) VALUES(:p1,:p2,:p3,:p4,:p5,:p6,''::bytea) ON CONFLICT(id) DO NOTHING;
