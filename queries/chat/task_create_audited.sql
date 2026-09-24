--: Record(goal_id?)

--! run (p1, p2, p3, p4, p5?, p6, p7) : Record
-- All dependency links are inserted as one set. Their scoped foreign keys
-- reject missing/foreign dependencies and roll back the entire transaction.
WITH created AS (
    INSERT INTO chat_tasks(id, thread_id, agent_id, title, goal_id)
    VALUES(:p1, :p2, :p3, :p4, :p5)
    ON CONFLICT(id) DO NOTHING RETURNING id, title, status, goal_id, blocked_reason
), linked AS (
    INSERT INTO chat_task_dependencies(thread_id, agent_id, task_id, dependency_id)
    SELECT :p2, :p3, created.id, dep FROM created CROSS JOIN UNNEST(:p6::UUID[]) AS dep
    RETURNING dependency_id
), selected AS MATERIALIZED (
    SELECT c.id, c.title, c.status, c.goal_id, c.blocked_reason,
           ARRAY(SELECT dependency_id FROM linked ORDER BY dependency_id) AS dependencies
    FROM created c
    UNION ALL
    SELECT t.id, t.title, t.status, t.goal_id, t.blocked_reason,
           ARRAY(SELECT dependency_id FROM chat_task_dependencies WHERE task_id = t.id ORDER BY dependency_id)
    FROM chat_tasks t
    WHERE t.id = :p1 AND t.thread_id = :p2 AND t.agent_id = :p3 AND NOT EXISTS(SELECT 1 FROM created)
), valid AS MATERIALIZED (
    SELECT * FROM selected WHERE title = :p4 AND goal_id IS NOT DISTINCT FROM :p5 AND dependencies = :p6
), advanced AS (
    UPDATE chat_threads SET activity_sequence = :p7::BIGINT
    WHERE id = :p2 AND activity_sequence = :p7::BIGINT - 1 AND EXISTS(SELECT 1 FROM valid)
    RETURNING id
)
SELECT v.id AS id, v.title AS title, v.status AS status, v.goal_id,
       v.blocked_reason AS blocked_reason, v.dependencies AS dependencies, txid_current() AS transaction_id
FROM valid v WHERE EXISTS(SELECT 1 FROM advanced);
