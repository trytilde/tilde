--! run (p1, p2)
UPDATE agents SET tool_mode=:p2 WHERE id=:p1;
