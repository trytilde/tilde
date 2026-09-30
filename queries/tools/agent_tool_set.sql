--! run (p1, p2, p3, p4, p5, p6, p7)
INSERT INTO agent_tools(source_id,tool_name,name,is_async,summary,description,display) VALUES(:p1,:p2,:p3,:p4,:p5,:p6,:p7)
ON CONFLICT(source_id,tool_name) DO UPDATE SET is_async=excluded.is_async,summary=excluded.summary,
  description=excluded.description,display=excluded.display;
