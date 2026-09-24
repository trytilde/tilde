--: Record(goal_id?)

--! run (p1, p2) : Record
SELECT t.id,t.title,t.status,t.goal_id,t.blocked_reason,ARRAY(SELECT dependency_id FROM chat_task_dependencies WHERE task_id=t.id ORDER BY dependency_id) AS dependencies FROM chat_tasks t WHERE thread_id=:p1 AND agent_id=:p2 ORDER BY id;
