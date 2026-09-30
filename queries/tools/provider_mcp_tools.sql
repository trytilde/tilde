--: Record()

-- Personal connections' snapshots are their owner's alone (descriptions and enums can name the
-- account), so the shared catalog only reads installation connections.
--! run (p1) : Record
SELECT DISTINCT ON (t.name) t.name,t.description,t.input_schema_json,t.output_schema_json,t.read_only,t.destructive,t.idempotent,t.open_world FROM connection_tools t JOIN connections c ON c.id=t.connection_id
WHERE c.provider_id=:p1 AND c.owner_user_id IS NULL
ORDER BY t.name;
