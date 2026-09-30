--: Record()

--! run (p1) : Record
SELECT name,description,input_schema_json,output_schema_json,read_only,destructive,idempotent,open_world FROM connection_tools WHERE connection_id=:p1 ORDER BY name;
