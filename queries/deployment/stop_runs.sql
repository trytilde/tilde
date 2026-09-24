--: Record()

--! run (p1, p2) : Record
UPDATE chat_runs SET status='failed' WHERE thread_id=:p1 AND agent_id=:p2 AND status IN ('active','suspending') RETURNING id;
