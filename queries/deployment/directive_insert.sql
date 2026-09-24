--! run (p1, p2, p3, p4?, p5)
INSERT INTO sidecar_directives(id,agent_id,instance_id,thread_id,payload) VALUES(:p1,:p2,:p3,:p4,:p5);
