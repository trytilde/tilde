SELECT id,active FROM chat_participants WHERE thread_id=$1 AND (user_id=$2 OR agent_id=$3) FOR UPDATE;
