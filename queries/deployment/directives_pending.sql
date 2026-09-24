--: Record(thread_id?)

--! run (p1, p2) : Record
SELECT id,thread_id,payload FROM sidecar_directives WHERE agent_id=:p1 AND instance_id=:p2 AND acked_at IS NULL ORDER BY created_at,id;
