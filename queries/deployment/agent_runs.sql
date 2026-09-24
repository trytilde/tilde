--: Record(goal_id?, invocation_id?, invocation_status?)

--! run (p1, p2) : Record
SELECT r.id,r.thread_id,r.agent_id,r.objective,r.status,r.goal_id,i.id AS invocation_id,i.status AS invocation_status
FROM chat_runs r
LEFT JOIN LATERAL (SELECT id,status FROM chat_invocations WHERE run_id=r.id ORDER BY started_at DESC NULLS FIRST LIMIT 1) i ON TRUE
WHERE r.thread_id=:p1 AND r.agent_id=:p2 AND r.status IN ('active','suspending','waiting') ORDER BY r.id;
