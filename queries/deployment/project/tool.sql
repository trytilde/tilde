--! run (p1, p2, p3, p4, p5, p6, p7, p8, p9, p10)
INSERT INTO chat_tool_calls(id,thread_id,invocation_id,participant_id,name,provider_id,status,input_json,output_json,error) VALUES(:p1,:p2,:p3,:p4,:p5,:p6,:p7,:p8,:p9,:p10) ON CONFLICT(id) DO UPDATE SET status=EXCLUDED.status,output_json=EXCLUDED.output_json,error=EXCLUDED.error,updated_at=NOW();
