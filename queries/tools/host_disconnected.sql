--! run (p1)
UPDATE tool_hosts SET connected_at=NULL WHERE id=:p1;
