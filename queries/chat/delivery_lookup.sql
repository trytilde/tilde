SELECT d.message_id FROM chat_message_deliveries d JOIN chat_messages m ON m.id=d.message_id WHERE d.connection_id=$1 AND d.external_message_id=$2 AND m.thread_id=$3;
