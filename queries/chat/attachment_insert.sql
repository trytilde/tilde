INSERT INTO chat_attachments(id,thread_id,filename,media_type,size_bytes,sha256,content) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT DO NOTHING;
