--! run (p1, p2, p3)
INSERT INTO mcp_server_health(provider_id,checked_at,healthy) VALUES(:p1,:p2,:p3);
