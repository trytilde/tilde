--! run (p1, p2, p3, p4, p5, p6?, p7?, p8?, p9?)
INSERT INTO tool_host_calls(id,tool_host_id,kind,name,input_json,agent_id,thread_id,connection_id,credentials) VALUES(:p1,:p2,:p3,:p4,:p5,:p6,:p7,:p8,:p9);
