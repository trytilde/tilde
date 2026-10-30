--: Record(valid_from?, valid_to?, identity_id?, agent_id?)

--! run (p1) : Record
SELECT at,operation,kind,path,title,content,lower(valid_during) AS valid_from,upper(valid_during) AS valid_to,
  importance,origin,identity_id,actor,agent_id
FROM memory_revisions WHERE memory_id=:p1 ORDER BY id;
