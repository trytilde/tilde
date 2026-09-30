--! run (p1)
UPDATE tool_hosts SET connected_at=NOW() WHERE id=:p1;
