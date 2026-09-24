--: Record()

--! run (p1, p2) : Record
SELECT a.id, hours.hour_start AS hour_start,
    COUNT(h.id)::BIGINT AS total_checks,
    COUNT(h.id) FILTER (WHERE NOT h.healthy)::BIGINT AS failed_checks,
    COUNT(h.id) FILTER (WHERE h.degraded)::BIGINT AS degraded_checks
FROM agents a CROSS JOIN generate_series(
    date_trunc('hour', :p2::TIMESTAMPTZ, 'UTC') - INTERVAL '11 hours',
    date_trunc('hour', :p2::TIMESTAMPTZ, 'UTC'), INTERVAL '1 hour'
) AS hours(hour_start)
LEFT JOIN agent_health h ON h.agent_id = a.id AND h.sidecar_event_id IS NULL
    AND h.checked_at >= hours.hour_start AND h.checked_at < hours.hour_start + INTERVAL '1 hour'
    AND h.checked_at <= :p2
WHERE a.id = ANY(:p1)
GROUP BY a.id, hours.hour_start ORDER BY a.id, hours.hour_start;
