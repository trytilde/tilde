DELETE FROM agent_health WHERE checked_at < $1::TIMESTAMPTZ - INTERVAL '14 days';
