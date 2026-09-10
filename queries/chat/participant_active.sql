UPDATE chat_participants SET active=$3 WHERE thread_id=$1 AND id=$2 AND active<>$3;
