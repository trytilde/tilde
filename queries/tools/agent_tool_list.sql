--: Record()

--! run (p1) : Record
SELECT source_id,tool_name,name,is_async,summary,description,display FROM agent_tools WHERE source_id=ANY(:p1) ORDER BY source_id,name;
