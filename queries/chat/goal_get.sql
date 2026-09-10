SELECT id,objective,status FROM chat_goals WHERE thread_id=$1 AND agent_id=$2 AND id=$3;
