--! run (p1)
DELETE FROM mcp_server_health WHERE checked_at < :p1::TIMESTAMPTZ - INTERVAL '14 days';
