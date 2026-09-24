--: Record()

--! run (p1, p2) : Record
UPDATE sidecar_directives SET acked_at=NOW() WHERE id=:p1 AND agent_id=:p2 AND acked_at IS NULL RETURNING id;
