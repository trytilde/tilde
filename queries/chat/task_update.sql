--: Record(goal_id?)

--! run (p1, p2, p3, p4, p5) : Record
UPDATE chat_tasks t SET status=:p4,blocked_reason=:p5 WHERE thread_id=:p1 AND agent_id=:p2 AND id=:p3 AND (status IN ('pending','working','blocked') OR status=:p4) RETURNING t.id,t.title,t.status,t.goal_id,t.blocked_reason,ARRAY(SELECT dependency_id FROM chat_task_dependencies WHERE task_id=t.id ORDER BY dependency_id) AS dependencies;
