--: Record()

--! run (p1) : Record
SELECT name,description,summary,input_schema_json,output_schema_json,read_only,destructive,idempotent,open_world,display FROM invocation_bundled_tools WHERE invocation_id=:p1 ORDER BY name;
