--: Record(object_key?, content?, connection_id?, provider_attachment_id?, source_url?)

--! run (p1, p2) : Record
SELECT object_key,id,thread_id,filename,media_type,size_bytes,sha256,content,connection_id,provider_attachment_id,source_url FROM chat_attachments WHERE id=:p1 AND thread_id=:p2;
