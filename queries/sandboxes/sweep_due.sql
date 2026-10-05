--: Record()

--! run : Record
-- Unleased sandboxes the sweeper acts on, by their blueprint's timings. `terminate`: unused past
-- terminate_after, failed an hour ago, or no longer wanted: the blueprint now reuses sandboxes
-- differently or launches them with another connection, or the agent it was launched for (any
-- agent, for a global one) no longer has the blueprint as its sandbox. `sleep`: running and unused
-- past sleep_after.
SELECT id,action FROM (
  SELECT s.id, CASE
    WHEN s.reuse <> b.reuse OR s.connection_id <> b.connection_id
      OR s.last_used_at < NOW() - make_interval(secs => b.terminate_after_secs)
      OR (s.status='failed' AND s.last_used_at < NOW() - INTERVAL '1 hour')
      OR NOT EXISTS (SELECT 1 FROM agent_sandboxes a WHERE a.blueprint_id=s.blueprint_id AND (s.agent_id IS NULL OR a.agent_id=s.agent_id))
    THEN 'terminate'
    WHEN s.status='running' AND s.last_used_at < NOW() - make_interval(secs => b.sleep_after_secs) THEN 'sleep'
  END AS action
  FROM sandboxes s JOIN sandbox_blueprints b ON b.id=s.blueprint_id
  WHERE s.lease_until IS NULL OR s.lease_until <= NOW()
) due WHERE action IS NOT NULL LIMIT 50;
