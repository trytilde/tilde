--: Record()

--! run (p1, p2, p3, p4) : Record
UPDATE chat_goals SET status=:p4 WHERE id=:p3 AND thread_id=:p1 AND agent_id=:p2 AND (status='active' OR status=:p4) RETURNING id,objective,status;
