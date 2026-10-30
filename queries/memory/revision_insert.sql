--! run (p1, p2, p3, p4?, p5?)
-- Snapshot the memory as it now is, made by actor p3 (an agent p4, a person, or the synthesizer in
-- batch p5).
INSERT INTO memory_revisions(memory_id,operation,kind,path,title,content,valid_during,importance,origin,identity_id,actor,agent_id,batch_id)
SELECT id,:p2,kind,path,title,content,valid_during,importance,origin,identity_id,:p3,:p4,:p5 FROM memories WHERE id=:p1;
