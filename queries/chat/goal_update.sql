UPDATE chat_goals SET status=$4 WHERE id=$3 AND thread_id=$1 AND agent_id=$2 AND (status='active' OR status=$4) RETURNING id,objective,status;
