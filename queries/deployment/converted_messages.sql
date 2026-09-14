SELECT c.message_id,c.representation FROM chat_converted_messages c JOIN chat_messages m ON m.id=c.message_id WHERE c.agent_id=$1 AND m.thread_id=$2 ORDER BY c.message_id;
