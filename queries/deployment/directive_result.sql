--: Record()

--! run (p1, p2, p3, p4) : Record
UPDATE sidecar_directives SET acked_at=NOW(),result=:p4 WHERE id=:p1 AND agent_id=:p2 AND instance_id=:p3 AND acked_at IS NULL RETURNING id;
