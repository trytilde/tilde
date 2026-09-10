INSERT INTO chat_attachments(id,thread_id,filename,media_type,size_bytes,sha256,connection_id,provider_attachment_id,source_url) VALUES($1,$2,$3,$4,$5,'',$6,$7,$8) ON CONFLICT DO NOTHING;
