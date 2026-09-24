-- Health is sampled independently from invocation/thread state.
CREATE TABLE agent_health (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    endpoint_url TEXT NOT NULL,
    checked_at TIMESTAMPTZ NOT NULL,
    healthy BOOLEAN NOT NULL,
    latency_ms INTEGER NOT NULL CHECK (latency_ms >= 0),
    error_code TEXT CHECK (error_code IN ('timeout', 'transport', 'not_ready', 'credentials')),
    CHECK ((healthy AND error_code IS NULL) OR (NOT healthy AND error_code IS NOT NULL))
);
CREATE INDEX agent_health_latest ON agent_health(agent_id, checked_at DESC, id DESC);
CREATE INDEX agent_health_retention ON agent_health(checked_at);

-- Capture first visible content, including streamed replies. Old messages remain unmeasured.
ALTER TABLE chat_messages ADD COLUMN first_content_at TIMESTAMPTZ;
CREATE FUNCTION chat_capture_first_content() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.first_content_at IS NULL AND length(NEW.text) > 0 THEN
        NEW.first_content_at := clock_timestamp();
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER chat_first_content BEFORE INSERT OR UPDATE OF text ON chat_messages
    FOR EACH ROW EXECUTE FUNCTION chat_capture_first_content();
CREATE INDEX chat_agent_sessions ON chat_participants(agent_id, thread_id) WHERE agent_id IS NOT NULL;
CREATE INDEX chat_agent_messages ON chat_messages(participant_id);
CREATE INDEX chat_invocation_first_content ON chat_messages(invocation_id, first_content_at)
    WHERE invocation_id IS NOT NULL AND first_content_at IS NOT NULL;
