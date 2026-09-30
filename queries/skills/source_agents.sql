--: Record()

--! run (id) : Record
SELECT agent_id FROM agent_skill_sources WHERE source_id=:id AND enabled ORDER BY created_at;
