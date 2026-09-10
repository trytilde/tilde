WITH selected AS (SELECT id, endpoint_url FROM agents WHERE id = ANY($1)),
sessions AS (
    SELECT p.agent_id, COUNT(*) AS count
    FROM chat_participants p JOIN selected a ON a.id = p.agent_id GROUP BY p.agent_id
), turns AS (
    SELECT p.agent_id, COUNT(*) AS count FROM chat_messages m
    JOIN chat_participants p ON p.id = m.participant_id JOIN selected a ON a.id = p.agent_id
    WHERE m.status = 'complete' AND length(m.text) > 0 GROUP BY p.agent_id
), first_replies AS (
    SELECT i.id, i.agent_id, i.started_at, MIN(m.first_content_at) AS first_content_at
    FROM chat_invocations i JOIN selected a ON a.id = i.agent_id
    JOIN chat_messages m ON m.invocation_id = i.id
    JOIN chat_participants p ON p.id = m.participant_id AND p.agent_id = i.agent_id
    WHERE i.started_at IS NOT NULL AND m.first_content_at >= i.started_at
    GROUP BY i.id, i.agent_id, i.started_at
), responses AS (
    SELECT agent_id, AVG(EXTRACT(EPOCH FROM (first_content_at - started_at)) * 1000)::DOUBLE PRECISION AS avg_ms
    FROM first_replies GROUP BY agent_id
)
SELECT a.id,
    COALESCE(s.count, 0)::BIGINT AS "thread_count!",
    (COALESCE(t.count, 0)::DOUBLE PRECISION / NULLIF(s.count, 0)) AS average_turns_per_thread,
    r.avg_ms AS average_response_ms,
    h.healthy, h.checked_at
FROM selected a LEFT JOIN sessions s ON s.agent_id = a.id
LEFT JOIN turns t ON t.agent_id = a.id LEFT JOIN responses r ON r.agent_id = a.id
LEFT JOIN LATERAL (
    SELECT healthy, checked_at FROM agent_health
    WHERE agent_id = a.id AND endpoint_url = a.endpoint_url AND checked_at <= $2
    ORDER BY checked_at DESC, id DESC LIMIT 1
) h ON TRUE;
