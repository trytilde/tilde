--: Record()

--! run (p1) : Record
DELETE FROM tool_host_calls WHERE id=:p1 AND status IN ('completed','failed') RETURNING status,output_json,error;
