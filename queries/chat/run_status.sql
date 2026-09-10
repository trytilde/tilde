UPDATE chat_runs SET status=$2 WHERE id=$1 AND status IN ('active','waiting');
