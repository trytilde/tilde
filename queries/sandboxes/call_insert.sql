--! run (p1, p2, p3, p4, p5, p6, p7)
INSERT INTO sandbox_calls(id,sandbox_id,operation,input_json,invocation_id,traceparent,tracestate) VALUES(:p1,:p2,:p3,:p4,:p5,:p6,:p7);
