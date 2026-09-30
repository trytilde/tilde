--: Record()

--! run (p1) : Record
SELECT version_id,name,hash,content FROM prompt_sections WHERE version_id=ANY(:p1::UUID[]) ORDER BY version_id,name;