-- One durable pending version per session. Allocate update versions after the
-- conflict row is locked so concurrent transactions cannot move a session backward.
CREATE SEQUENCE chat_session_projection_version;
CREATE TABLE chat_session_projection_pending (
 thread_id UUID PRIMARY KEY,
 version BIGINT NOT NULL DEFAULT nextval('chat_session_projection_version') CHECK(version>0)
);
CREATE INDEX chat_session_projection_pending_order ON chat_session_projection_pending(version);
CREATE FUNCTION chat_session_projection_mark(target UUID) RETURNS VOID LANGUAGE SQL AS $$
 INSERT INTO chat_session_projection_pending(thread_id) VALUES(target)
 ON CONFLICT(thread_id) DO UPDATE SET version=nextval('chat_session_projection_version');
$$;
CREATE FUNCTION chat_session_projection_child() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP<>'INSERT' THEN PERFORM chat_session_projection_mark(OLD.thread_id); END IF;
 IF TG_OP='INSERT' OR (TG_OP='UPDATE' AND NEW.thread_id IS DISTINCT FROM OLD.thread_id) THEN
  PERFORM chat_session_projection_mark(NEW.thread_id);
 END IF;
 RETURN NULL;
END;
$$;
CREATE FUNCTION chat_session_projection_thread() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN PERFORM chat_session_projection_mark(OLD.id);
 ELSE PERFORM chat_session_projection_mark(NEW.id); END IF;
 RETURN NULL;
END;
$$;
CREATE TRIGGER chat_session_projection_thread AFTER INSERT OR DELETE ON chat_threads FOR EACH ROW EXECUTE FUNCTION chat_session_projection_thread();
CREATE TRIGGER chat_session_projection_message AFTER INSERT OR DELETE ON chat_messages FOR EACH ROW EXECUTE FUNCTION chat_session_projection_child();
-- Streaming chunks do not change counts once the first nonempty content is present.
CREATE TRIGGER chat_session_projection_message_change AFTER UPDATE OF text,status,created_at ON chat_messages
 FOR EACH ROW WHEN (OLD.status IS DISTINCT FROM NEW.status OR OLD.created_at IS DISTINCT FROM NEW.created_at OR (length(OLD.text)=0) IS DISTINCT FROM (length(NEW.text)=0)) EXECUTE FUNCTION chat_session_projection_child();
CREATE TRIGGER chat_session_projection_attachment AFTER INSERT OR UPDATE OR DELETE ON chat_message_attachments FOR EACH ROW EXECUTE FUNCTION chat_session_projection_child();
CREATE TRIGGER chat_session_projection_participant AFTER INSERT OR UPDATE OF thread_id,user_id,agent_id OR DELETE ON chat_participants FOR EACH ROW EXECUTE FUNCTION chat_session_projection_child();
CREATE TRIGGER chat_session_projection_channel AFTER INSERT OR UPDATE OR DELETE ON chat_channel_threads FOR EACH ROW EXECUTE FUNCTION chat_session_projection_child();
CREATE TRIGGER chat_session_projection_turn AFTER INSERT OR DELETE ON chat_invocations FOR EACH ROW EXECUTE FUNCTION chat_session_projection_child();
CREATE TRIGGER chat_session_projection_turn_start AFTER UPDATE OF started_at ON chat_invocations FOR EACH ROW WHEN(OLD.started_at IS DISTINCT FROM NEW.started_at) EXECUTE FUNCTION chat_session_projection_child();
CREATE FUNCTION chat_session_projection_identity() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE target UUID;
BEGIN
 IF TG_OP='DELETE' THEN target:=OLD.id; ELSE target:=NEW.id; END IF;
 INSERT INTO chat_session_projection_pending(thread_id)
 SELECT DISTINCT thread_id FROM chat_participants WHERE user_id=target ORDER BY thread_id
 ON CONFLICT(thread_id) DO UPDATE SET version=nextval('chat_session_projection_version');
 RETURN NULL;
END;
$$;
CREATE TRIGGER chat_session_projection_identity AFTER INSERT OR UPDATE OF connection_id OR DELETE ON chat_channel_identities FOR EACH ROW EXECUTE FUNCTION chat_session_projection_identity();
CREATE FUNCTION chat_session_projection_provider() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 INSERT INTO chat_session_projection_pending(thread_id)
 SELECT b.thread_id FROM chat_channel_threads b JOIN connections c ON c.id=b.connection_id WHERE c.provider_id=NEW.provider_id ORDER BY b.thread_id
 ON CONFLICT(thread_id) DO UPDATE SET version=nextval('chat_session_projection_version');
 RETURN NULL;
END;
$$;
CREATE TRIGGER chat_session_projection_provider AFTER UPDATE OF name,icon_url ON connection_providers FOR EACH ROW WHEN(OLD.name IS DISTINCT FROM NEW.name OR OLD.icon_url IS DISTINCT FROM NEW.icon_url) EXECUTE FUNCTION chat_session_projection_provider();
CREATE FUNCTION chat_session_projection_connection() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 INSERT INTO chat_session_projection_pending(thread_id) SELECT thread_id FROM chat_channel_threads WHERE connection_id=NEW.id ORDER BY thread_id
 ON CONFLICT(thread_id) DO UPDATE SET version=nextval('chat_session_projection_version');
 RETURN NULL;
END;
$$;
CREATE TRIGGER chat_session_projection_connection AFTER UPDATE OF provider_id ON connections FOR EACH ROW WHEN(OLD.provider_id IS DISTINCT FROM NEW.provider_id) EXECUTE FUNCTION chat_session_projection_connection();
CREATE FUNCTION chat_session_projection_agent() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 INSERT INTO chat_session_projection_pending(thread_id) SELECT thread_id FROM chat_participants WHERE agent_id=NEW.id ORDER BY thread_id
 ON CONFLICT(thread_id) DO UPDATE SET version=nextval('chat_session_projection_version');
 RETURN NULL;
END;
$$;
CREATE TRIGGER chat_session_projection_agent AFTER UPDATE OF deleted_at ON agents FOR EACH ROW WHEN(OLD.deleted_at IS DISTINCT FROM NEW.deleted_at) EXECUTE FUNCTION chat_session_projection_agent();
INSERT INTO chat_session_projection_pending(thread_id) SELECT id FROM chat_threads;
