--: Record()

--! run (p1) : Record
DELETE FROM sandbox_calls WHERE id=:p1 AND status IN ('completed','failed') RETURNING status,output_json,error;
