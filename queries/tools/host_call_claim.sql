--: Record(agent_id?, thread_id?, connection_id?, credentials?, connection_type?, sandbox_id?)

--! run (p1) : Record
-- Several processes may hold a Watch stream for one host; each pending call goes to one of them.
UPDATE tool_host_calls SET status='delivered' WHERE id IN (
  SELECT id FROM tool_host_calls WHERE tool_host_id=:p1 AND status='pending' ORDER BY created_at FOR UPDATE SKIP LOCKED
) RETURNING id,kind,name,input_json,agent_id,thread_id,connection_id,credentials,sandbox_id,
  (SELECT type_id FROM connections c WHERE c.id=tool_host_calls.connection_id) AS connection_type;
