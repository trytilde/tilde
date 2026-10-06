--: Record(agent_id?, thread_id?, identity_id?, provider_sandbox_id?)

--! run (p1?, p2?, p3) : Record
-- A blueprint's (p1) or an agent's (p2) sandboxes; `connected` judged against p3 seconds.
SELECT id,blueprint_id,reuse,agent_id,thread_id,identity_id,status,provider_sandbox_id,error,last_used_at,created_at,
  COALESCE(connected_at > NOW() - make_interval(secs => :p3), false) AS connected
FROM sandboxes WHERE (:p1::UUID IS NULL OR blueprint_id=:p1) AND (:p2::UUID IS NULL OR agent_id=:p2)
ORDER BY last_used_at DESC;
