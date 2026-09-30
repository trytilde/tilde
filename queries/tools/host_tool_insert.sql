--! run (p1, p2, p3, p4, p5, p6, p7, p8, p9, p10)
INSERT INTO tool_host_tools(tool_host_id,name,description,summary,input_schema_json,output_schema_json,read_only,destructive,idempotent,open_world) VALUES(:p1,:p2,:p3,:p4,:p5,:p6,:p7,:p8,:p9,:p10);
