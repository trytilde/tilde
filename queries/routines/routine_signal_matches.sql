--: Record()

--! run (connection, signal_type) : Record
SELECT r.id,r.agent_id,r.name,r.prompt FROM routines r
JOIN agents a ON a.id=r.agent_id AND a.deleted_at IS NULL
WHERE r.connection_id=:connection AND r.signal_type=:signal_type AND r.enabled;
