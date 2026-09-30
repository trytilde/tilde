--: Record()

-- A stamp names a version by the agent's prompt name and content hash.
--! run (agent_id, name, hash) : Record
SELECT v.id FROM prompts p JOIN prompt_versions v ON v.prompt_id=p.id WHERE p.agent_id=:agent_id AND p.name=:name AND v.hash=:hash;
