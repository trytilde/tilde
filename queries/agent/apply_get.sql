--: Record(key?)

--! run (key, id?) : Record
-- The agent declarative apply upserts: the one with this key, else the one with this ID.
SELECT id, key, name, description, concurrency_policy, tool_mode, capabilities AS capabilities
FROM agents
WHERE deleted_at IS NULL AND (key = :key OR id = :id)
ORDER BY key = :key DESC NULLS LAST LIMIT 1;
