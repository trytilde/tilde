--: Record(exhausted_until?, connection_id?)

--! run (p1, p2, p3, p4?, p5, p6, p7) : Record
INSERT INTO inference_budgets(id,scope,scope_id,connection_id,period,limit_micros,action) VALUES(:p1,:p2,:p3,:p4,:p5,:p6,:p7)
ON CONFLICT (scope,scope_id,period,COALESCE(connection_id,'00000000-0000-0000-0000-000000000000')) DO UPDATE SET limit_micros=excluded.limit_micros,action=excluded.action,exhausted_until=NULL,updated_at=NOW()
RETURNING id,scope,scope_id,connection_id,period,limit_micros,action,spent_micros,exhausted_until,created_at,updated_at;
