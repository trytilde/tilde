-- Recovery woke on every write to agent_instances, so every heartbeat (which usually only moves
-- last_seen_at) sent a NOTIFY. A committing NOTIFY holds the server-wide notification queue lock
-- through its WAL flush, so heartbeats committed one at a time, queued behind every other
-- notifying commit on the server (another application's included), and held their pool
-- checkouts meanwhile. A fresher last_seen_at never makes a lease dead; replicas that stop
-- heartbeating are found by recovery's interval. Registration and changes of connection or
-- readiness still wake it.
DROP TRIGGER sidecar_recovery_health ON agent_instances;
CREATE TRIGGER sidecar_recovery_registered AFTER INSERT ON agent_instances
FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_recovery');
CREATE TRIGGER sidecar_recovery_health AFTER UPDATE ON agent_instances
FOR EACH ROW WHEN (OLD.connection_id IS DISTINCT FROM NEW.connection_id OR
 OLD.deployment_id IS DISTINCT FROM NEW.deployment_id OR
 OLD.ready IS DISTINCT FROM NEW.ready OR OLD.agent_ready IS DISTINCT FROM NEW.agent_ready OR
 OLD.agent_connected IS DISTINCT FROM NEW.agent_connected)
EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_recovery');
