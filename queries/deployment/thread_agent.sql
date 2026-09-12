SELECT EXISTS(SELECT 1 FROM chat_participants WHERE thread_id=$1 AND agent_id=$2 AND active) AS "allowed!";
