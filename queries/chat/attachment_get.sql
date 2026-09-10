SELECT id,thread_id,filename,media_type,size_bytes,sha256,content,connection_id,provider_attachment_id,source_url FROM chat_attachments WHERE id=$1 AND thread_id=$2;
