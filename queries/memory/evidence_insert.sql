--! run (p1, p2, p3?, p4?, p5?, p6?)
INSERT INTO memory_evidence(memory_id,batch_id,thread_id,message_id,page_id,revision) VALUES(:p1,:p2,:p3,:p4,:p5,:p6);
