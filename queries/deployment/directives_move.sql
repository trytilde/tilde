--! run (p1, p2, p3)
UPDATE sidecar_directives SET instance_id=:p3 WHERE thread_id=:p1 AND instance_id=:p2 AND acked_at IS NULL;
