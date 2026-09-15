UPDATE chat_runs r SET objective=r.objective || E'\n' || $2
FROM chat_invocations i WHERE i.id=$1 AND i.run_id=r.id AND i.status='pending';
