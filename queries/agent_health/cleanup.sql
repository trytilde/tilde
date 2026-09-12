DELETE FROM agent_health WHERE sidecar_event_id IS NULL AND checked_at < $1::TIMESTAMPTZ - INTERVAL '14 days';
