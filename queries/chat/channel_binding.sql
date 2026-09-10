SELECT b.connection_id,b.external_id,c.provider_id FROM chat_channel_threads b JOIN connections c ON c.id=b.connection_id WHERE b.thread_id=$1;
