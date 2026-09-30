--: Record(avatar_key?)

-- Filters apply before the keyset limit. `health` ('' for any) mirrors agent::health::metrics:
-- the latest routing sample, unknown once older than `fresh_after`.
--! run (p1?, p2?, p3, p4, health, paused?, fresh_after) : Record
SELECT a.id, a.name, a.description, a.concurrency_policy AS concurrency_policy, a.avatar_seed, a.avatar_key, a.paused,
       a.created_at, a.updated_at, a.capabilities AS capabilities
FROM agents a
LEFT JOIN LATERAL (
    SELECT h.healthy, h.degraded FROM agent_health h
    WHERE h.agent_id = a.id AND h.sidecar_event_id IS NULL AND h.checked_at >= :fresh_after
    ORDER BY h.checked_at DESC, h.id DESC LIMIT 1
) latest ON :health <> ''
WHERE a.deleted_at IS NULL AND (:p1::TIMESTAMPTZ IS NULL OR (a.created_at, a.id) < (:p1, :p2))
AND (:p4 = '' OR strpos(lower(a.name), lower(:p4)) > 0 OR strpos(lower(a.description), lower(:p4)) > 0 OR a.id::text = :p4)
AND (:paused::BOOLEAN IS NULL OR a.paused = :paused)
AND (:health = '' OR :health = CASE
    WHEN latest.healthy IS NULL THEN 'unknown'
    WHEN latest.degraded THEN 'degraded'
    WHEN latest.healthy THEN 'healthy'
    ELSE 'unhealthy' END)
ORDER BY a.created_at DESC, a.id DESC
LIMIT :p3;
