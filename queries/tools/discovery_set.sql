--! run (p1, p2)
INSERT INTO connection_tool_discovery(connection_id,toolset_hash) VALUES(:p1,:p2)
ON CONFLICT(connection_id) DO UPDATE SET toolset_hash=excluded.toolset_hash,discovered_at=NOW();
