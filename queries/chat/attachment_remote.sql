--! run (p1, p2, p3, p4, p5, p6, p7?, p8?)
INSERT INTO chat_attachments(id,thread_id,filename,media_type,size_bytes,sha256,connection_id,provider_attachment_id,source_url) VALUES(:p1,:p2,:p3,:p4,:p5,'',:p6,:p7,:p8) ON CONFLICT DO NOTHING;
