SELECT i.status,r.status AS run_status FROM chat_invocations i JOIN chat_runs r ON r.id=i.run_id WHERE i.id=$1 AND i.agent_id=$2 AND i.thread_id=$3 AND i.run_id=$4;
