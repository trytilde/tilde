-- A Watch generation proves the control connection exists; heartbeats alone do not.
ALTER TABLE agent_instances ADD COLUMN connection_id UUID;
ALTER TABLE agent_instances ADD COLUMN agent_connected BOOLEAN NOT NULL DEFAULT false;

-- Wake queued messages when an agent connection becomes usable or closes.
CREATE TRIGGER agent_connection_route_notify AFTER UPDATE ON agent_instances
FOR EACH ROW WHEN (OLD.connection_id IS DISTINCT FROM NEW.connection_id OR
 OLD.ready IS DISTINCT FROM NEW.ready OR OLD.agent_ready IS DISTINCT FROM NEW.agent_ready OR
 OLD.agent_connected IS DISTINCT FROM NEW.agent_connected)
EXECUTE FUNCTION tilde_notify_change('tilde_chat_activity');
