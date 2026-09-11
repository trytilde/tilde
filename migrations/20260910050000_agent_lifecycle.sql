-- Pause is independent of host health. Generations fence delayed host requests.
ALTER TABLE agents ADD COLUMN paused BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN generation BIGINT NOT NULL DEFAULT 0 CHECK (generation >= 0),
    ADD COLUMN deleted_at TIMESTAMPTZ;
ALTER TABLE agents DROP CONSTRAINT agents_webhook_signing_key_check;
ALTER TABLE agents ADD CHECK ((deleted_at IS NULL AND octet_length(webhook_signing_key) >= 46)
    OR (deleted_at IS NOT NULL AND octet_length(webhook_signing_key) = 0));
CREATE TRIGGER agents_resumed AFTER UPDATE OF paused ON agents
    FOR EACH ROW WHEN (OLD.paused AND NOT NEW.paused)
    EXECUTE FUNCTION tilde_notify_change('tilde_chat_work');
