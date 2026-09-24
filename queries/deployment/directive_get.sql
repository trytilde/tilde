--: Record(acked_at?, result?)

--! run (p1) : Record
SELECT acked_at,result FROM sidecar_directives WHERE id=:p1;
