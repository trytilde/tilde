--: Record()

--! run (p1) : Record
SELECT tool_host_id,name,description,summary,input_schema_json,output_schema_json,read_only,destructive,idempotent,open_world FROM tool_host_tools WHERE tool_host_id=ANY(:p1) ORDER BY tool_host_id,name;
