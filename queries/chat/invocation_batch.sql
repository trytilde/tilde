--! run (p1, p2)
UPDATE chat_runs r SET objective=r.objective || E'\n' || :p2
FROM chat_invocations i WHERE i.id=:p1 AND i.run_id=r.id AND i.status='pending';
