--: Record()

--! run (p1) : Record
SELECT id,agent_id,name,created_at FROM prompts WHERE id=:p1;