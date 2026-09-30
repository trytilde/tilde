--: Record()

--! run (p1, p2) : Record
-- Twelve hourly buckets per host ending with the hour of p2, oldest first.
SELECT h.id, hours.hour_start AS hour_start,
    COUNT(s.id)::BIGINT AS total_checks,
    COUNT(s.id) FILTER (WHERE NOT s.healthy)::BIGINT AS failed_checks
FROM tool_hosts h CROSS JOIN generate_series(
    date_trunc('hour', :p2::TIMESTAMPTZ, 'UTC') - INTERVAL '11 hours',
    date_trunc('hour', :p2::TIMESTAMPTZ, 'UTC'), INTERVAL '1 hour'
) AS hours(hour_start)
LEFT JOIN tool_host_health s ON s.tool_host_id = h.id
    AND s.checked_at >= hours.hour_start AND s.checked_at < hours.hour_start + INTERVAL '1 hour'
    AND s.checked_at <= :p2
WHERE h.id = ANY(:p1)
GROUP BY h.id, hours.hour_start ORDER BY h.id, hours.hour_start;
