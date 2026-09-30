--: Record()

--! run (p1) : Record
SELECT id FROM tool_hosts WHERE token_hash=:p1 AND execution_type='connected';
