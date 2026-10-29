--! run (p1, p2, p3?)
-- Snapshot the memory as it now is.
INSERT INTO memory_revisions(memory_id,operation,kind,path,title,content,valid_during,importance,origin,identity_id,agent_id)
SELECT id,:p2,kind,path,title,content,valid_during,importance,origin,identity_id,:p3 FROM memories WHERE id=:p1;
