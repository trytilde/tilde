--! run (id, thread?, error?)
UPDATE routine_runs SET thread_id=:thread,error=:error WHERE id=:id;
