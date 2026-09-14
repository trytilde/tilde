SELECT i.id,i.value,i.identity_type,i.verified_at FROM chat_channel_identities i WHERE i.connection_id=$1 AND i.id=$2;
