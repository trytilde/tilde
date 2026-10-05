--: Record()

--! run (p1, p2, p3?, p4?, p5?) : Record
-- The sandbox with this key: blueprint, reuse mode, and the agent, thread and person it keys by.
SELECT id FROM sandboxes WHERE blueprint_id=:p1 AND reuse=:p2
  AND agent_id IS NOT DISTINCT FROM :p3 AND thread_id IS NOT DISTINCT FROM :p4 AND identity_id IS NOT DISTINCT FROM :p5;
