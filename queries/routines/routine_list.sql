--: Record(schedule?, connection_id?, signal_type?, next_run_at?, last_run_at?, last_thread_id?, last_error?)

-- An agent's routines, or one routine, with each one's latest firing.
--! run (agent?, id?) : Record
SELECT r.id,r.agent_id,r.name,r.prompt,r.enabled,r.schedule,r.connection_id,r.signal_type,r.next_run_at,r.created_at,r.updated_at,last.created_at AS last_run_at,last.thread_id AS last_thread_id,last.error AS last_error
FROM routines r LEFT JOIN LATERAL (SELECT created_at,thread_id,error FROM routine_runs WHERE routine_id=r.id ORDER BY created_at DESC LIMIT 1) last ON true
WHERE (:agent::UUID IS NULL OR r.agent_id=:agent) AND (:id::UUID IS NULL OR r.id=:id) ORDER BY r.created_at,r.id;
