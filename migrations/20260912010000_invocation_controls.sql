CREATE TABLE invocation_control_receipts (
 invocation_id UUID NOT NULL REFERENCES chat_invocations(id), command_id UUID NOT NULL,
 acknowledged_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), PRIMARY KEY(invocation_id,command_id)
);
CREATE TRIGGER invocation_control_state AFTER UPDATE ON chat_invocations FOR EACH ROW WHEN (OLD.status IS DISTINCT FROM NEW.status) EXECUTE FUNCTION tilde_notify_change('tilde_invocation_control');
CREATE TRIGGER invocation_control_inputs AFTER INSERT OR UPDATE ON chat_inputs FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_invocation_control');
CREATE TRIGGER invocation_control_runs AFTER UPDATE ON chat_runs FOR EACH ROW WHEN (OLD.status IS DISTINCT FROM NEW.status) EXECUTE FUNCTION tilde_notify_change('tilde_invocation_control');
CREATE TRIGGER invocation_control_agents AFTER UPDATE ON agents FOR EACH ROW WHEN (OLD.paused IS DISTINCT FROM NEW.paused OR OLD.generation IS DISTINCT FROM NEW.generation OR OLD.deleted_at IS DISTINCT FROM NEW.deleted_at) EXECUTE FUNCTION tilde_notify_change('tilde_invocation_control');
CREATE TRIGGER invocation_control_assignments AFTER UPDATE ON participant_assignments FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_invocation_control');

DROP TRIGGER chat_input_insert_notify ON chat_inputs;
DROP TRIGGER chat_input_retry_notify ON chat_inputs;

ALTER TABLE chat_runs DROP CONSTRAINT chat_runs_status_check;
ALTER TABLE chat_runs ADD CONSTRAINT chat_runs_status_check CHECK(status IN ('active','suspending','waiting','completed','failed','canceled'));

ALTER TABLE sidecar_commands ADD COLUMN input_id UUID;
