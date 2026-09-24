-- Notifications are commit-time wakeups. Durable rows and cursors remain the
-- source of truth, including after listener reconnection. Never publish secrets.
CREATE FUNCTION tilde_notify_change() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    PERFORM pg_notify(TG_ARGV[0], '');
    RETURN NULL;
END;
$$;

CREATE TRIGGER chat_activity_notify AFTER INSERT ON chat_activity
FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_chat_activity');

CREATE TRIGGER chat_message_insert_notify AFTER INSERT ON chat_messages
FOR EACH ROW WHEN (NEW.status = 'complete')
EXECUTE FUNCTION tilde_notify_change('tilde_chat_work');
CREATE TRIGGER chat_message_complete_notify AFTER UPDATE ON chat_messages
FOR EACH ROW WHEN (NEW.status = 'complete')
EXECUTE FUNCTION tilde_notify_change('tilde_chat_work');
CREATE TRIGGER chat_targets_notify AFTER INSERT OR UPDATE OR DELETE ON chat_message_targets
FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_chat_work');
CREATE TRIGGER chat_participant_insert_notify AFTER INSERT ON chat_participants
FOR EACH ROW WHEN (NEW.agent_id IS NOT NULL AND NEW.active)
EXECUTE FUNCTION tilde_notify_change('tilde_chat_work');
CREATE TRIGGER chat_participant_active_notify AFTER UPDATE ON chat_participants
FOR EACH ROW WHEN (NEW.agent_id IS NOT NULL AND NEW.active AND NOT OLD.active)
EXECUTE FUNCTION tilde_notify_change('tilde_chat_work');
CREATE TRIGGER chat_invocation_insert_notify AFTER INSERT ON chat_invocations
FOR EACH ROW WHEN (NEW.status = 'pending')
EXECUTE FUNCTION tilde_notify_change('tilde_chat_work');
CREATE TRIGGER chat_invocation_pending_notify AFTER UPDATE ON chat_invocations
FOR EACH ROW WHEN (NEW.status = 'pending' AND OLD.status IS DISTINCT FROM NEW.status)
EXECUTE FUNCTION tilde_notify_change('tilde_chat_work');

CREATE TRIGGER chat_input_insert_notify AFTER INSERT ON chat_inputs
FOR EACH ROW WHEN (NOT NEW.accepted)
EXECUTE FUNCTION tilde_notify_change('tilde_chat_input');
CREATE TRIGGER chat_input_retry_notify AFTER UPDATE ON chat_inputs
FOR EACH ROW WHEN (NOT NEW.accepted)
EXECUTE FUNCTION tilde_notify_change('tilde_chat_input');
