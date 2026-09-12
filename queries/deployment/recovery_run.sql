SELECT i.run_id,r.objective FROM chat_invocations i JOIN chat_runs r ON r.id=i.run_id WHERE i.id=$1;
