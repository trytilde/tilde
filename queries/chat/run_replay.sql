--: Record(goal_id?)

--! run (p1, p2, p3) : Record
SELECT id,objective,goal_id FROM chat_runs WHERE thread_id=:p1 AND agent_id=:p2 AND idempotency_key=:p3;
