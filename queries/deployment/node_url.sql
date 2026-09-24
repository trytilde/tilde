--: Record()

--! run (p1, p2) : Record
SELECT public_url FROM agent_instances WHERE agent_id=:p1 AND instance_id=:p2;
