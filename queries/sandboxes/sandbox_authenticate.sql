--: Record(agent_id?, thread_id?)

--! run (p1) : Record
SELECT id,blueprint_id,agent_id,thread_id FROM sandboxes WHERE session_hash=:p1;
