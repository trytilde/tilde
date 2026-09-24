-- Existing routes keep their current public behavior; newly assigned routes start private.
ALTER TABLE connection_agents ADD COLUMN access_mode TEXT NOT NULL DEFAULT 'private' CHECK (access_mode IN ('private','public','disabled'));
UPDATE connection_agents SET access_mode='public';
CREATE TABLE chat_channel_identities (
 id UUID PRIMARY KEY REFERENCES chat_users(id), connection_id UUID NOT NULL REFERENCES connections(id),
 identity_type TEXT NOT NULL CHECK(identity_type IN ('email','phone_number','username')), value TEXT NOT NULL, verified_at TIMESTAMPTZ, created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 UNIQUE(connection_id,identity_type,value), UNIQUE(connection_id,id)
);
CREATE TABLE chat_channel_identity_access (
 connection_id UUID NOT NULL, capability TEXT NOT NULL DEFAULT 'channel' CHECK(capability='channel'),
 agent_id UUID NOT NULL, identity_id UUID NOT NULL, allowed BOOLEAN NOT NULL DEFAULT FALSE,
 PRIMARY KEY(connection_id,agent_id,identity_id),
 FOREIGN KEY(connection_id,capability,agent_id) REFERENCES connection_agents(connection_id,capability,agent_id) ON DELETE CASCADE,
 FOREIGN KEY(connection_id,identity_id) REFERENCES chat_channel_identities(connection_id,id)
);
CREATE TABLE chat_identity_verifications (
 id UUID PRIMARY KEY, connection_id UUID NOT NULL, agent_id UUID NOT NULL, identity_id UUID NOT NULL,
 token_hash BYTEA NOT NULL CHECK(octet_length(token_hash)=32),
 status TEXT NOT NULL CHECK(status IN ('pending','delivered','failed','approved')),
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), expires_at TIMESTAMPTZ NOT NULL,
 FOREIGN KEY(connection_id,agent_id,identity_id) REFERENCES chat_channel_identity_access(connection_id,agent_id,identity_id) ON DELETE CASCADE
);
CREATE INDEX chat_verification_identity ON chat_identity_verifications(identity_id,created_at DESC);
CREATE INDEX chat_verification_expiry ON chat_identity_verifications(expires_at);
CREATE TABLE chat_channel_access_audit (
 connection_id UUID NOT NULL REFERENCES connections(id), event_id TEXT NOT NULL,
 agent_id UUID NOT NULL REFERENCES agents(id), identity_id UUID NOT NULL REFERENCES chat_channel_identities(id),
 access_mode TEXT NOT NULL, accepted BOOLEAN NOT NULL, received_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 PRIMARY KEY(connection_id,event_id)
);
ALTER TABLE chat_messages ADD COLUMN source_identity_id UUID REFERENCES chat_channel_identities(id);
ALTER TABLE chat_runs ADD COLUMN source_identity_id UUID REFERENCES chat_channel_identities(id), ADD COLUMN channel_origin BOOLEAN NOT NULL DEFAULT FALSE;
-- Recover the old automatic-routing convention once; new writes carry an explicit origin.
UPDATE chat_runs r SET channel_origin=TRUE WHERE EXISTS(SELECT 1 FROM chat_channel_threads b WHERE b.thread_id=r.thread_id)
AND (EXISTS(SELECT 1 FROM chat_messages m WHERE m.thread_id=r.thread_id AND m.id::text=r.idempotency_key) OR r.idempotency_key LIKE 'pending:%');

CREATE FUNCTION chat_identity_allowed(target_agent UUID, sender UUID) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$
 SELECT EXISTS(SELECT 1 FROM chat_channel_identities i
 JOIN connection_agents ca ON ca.connection_id=i.connection_id AND ca.agent_id=target_agent AND ca.capability='channel'
 JOIN connections c ON c.id=ca.connection_id AND c.status='ready'
 LEFT JOIN chat_channel_identity_access a ON a.connection_id=i.connection_id AND a.agent_id=ca.agent_id AND a.identity_id=i.id
 WHERE i.id=sender AND (ca.access_mode='public' OR (ca.access_mode='private' AND i.verified_at IS NOT NULL AND a.allowed)));
$$;
CREATE FUNCTION chat_channel_message_allowed(target_agent UUID, target_thread UUID, sender UUID) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$
 SELECT NOT EXISTS(SELECT 1 FROM chat_channel_threads WHERE thread_id=target_thread)
 OR EXISTS(SELECT 1 FROM chat_channel_threads b
 JOIN connection_agents ca ON ca.connection_id=b.connection_id AND ca.agent_id=target_agent AND ca.capability='channel'
 JOIN connections c ON c.id=ca.connection_id AND c.status='ready'
 WHERE b.thread_id=target_thread AND (ca.access_mode='public' OR (ca.access_mode='private' AND EXISTS(
   SELECT 1 FROM chat_channel_identities i WHERE i.id=sender AND i.connection_id=b.connection_id AND chat_identity_allowed(target_agent,sender)))));
$$;
CREATE TRIGGER channel_policy_changed AFTER UPDATE OF access_mode ON connection_agents FOR EACH ROW EXECUTE FUNCTION tilde_notify_change('tilde_chat_work');
CREATE TRIGGER channel_grant_changed AFTER INSERT OR UPDATE ON chat_channel_identity_access FOR EACH ROW EXECUTE FUNCTION tilde_notify_change('tilde_chat_work');

-- Native/management-initiated runs have no provider sender. A later outbound channel binding
-- must not revoke their existing runtime grant. Provider-triggered runs always carry a sender.
CREATE FUNCTION chat_channel_run_allowed(target_agent UUID, target_thread UUID, sender UUID, provider_triggered BOOLEAN) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$
 SELECT NOT provider_triggered OR chat_channel_message_allowed(target_agent,target_thread,sender);
$$;
