--! run (p1, p2)
UPDATE tool_hosts SET token_hash=:p2 WHERE id=:p1 AND execution_type='connected';
