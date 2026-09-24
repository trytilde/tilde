-- Management keys have the same installation-wide authority as OIDC users.
-- Only digests are persisted; the bearer secret is returned once at creation.
CREATE TABLE iam_api_keys (
 id UUID PRIMARY KEY,
 name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
 prefix TEXT NOT NULL,
 token_hash BYTEA NOT NULL UNIQUE CHECK (octet_length(token_hash)=32),
 created_by_user_id UUID REFERENCES iam_users(id),
 created_by_api_key_id UUID REFERENCES iam_api_keys(id),
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 revoked_at TIMESTAMPTZ,
 CHECK (num_nonnulls(created_by_user_id,created_by_api_key_id)=1)
);
CREATE TRIGGER management_keys_changed AFTER UPDATE OR DELETE ON iam_api_keys
 FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_management_keys');

-- Roots group identities without changing their conversation participants or addresses.
CREATE TABLE root_identities (id UUID PRIMARY KEY, created_at TIMESTAMPTZ NOT NULL DEFAULT NOW());
ALTER TABLE chat_users ADD COLUMN root_identity_id UUID REFERENCES root_identities(id),
 ADD COLUMN attested_at TIMESTAMPTZ,
 ADD COLUMN attested_by_user_id UUID REFERENCES iam_users(id),
 ADD COLUMN attested_by_api_key_id UUID REFERENCES iam_api_keys(id),
 ADD CONSTRAINT identity_attestation CHECK (
  (attested_at IS NULL AND num_nonnulls(attested_by_user_id,attested_by_api_key_id)=0) OR
  (attested_at IS NOT NULL AND num_nonnulls(attested_by_user_id,attested_by_api_key_id)=1)
 );
CREATE INDEX chat_users_root ON chat_users(root_identity_id) WHERE root_identity_id IS NOT NULL;
CREATE TABLE chat_native_identities (
 id UUID PRIMARY KEY REFERENCES chat_users(id),
 value TEXT NOT NULL UNIQUE CHECK (length(value) BETWEEN 1 AND 320)
);

-- Management attestation can satisfy channel verification, but never creates an allow grant.
CREATE OR REPLACE FUNCTION chat_identity_allowed(target_agent UUID, sender UUID) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$
 SELECT EXISTS(SELECT 1 FROM chat_channel_identities i
 JOIN chat_users u ON u.id=i.id
 JOIN connection_agents ca ON ca.connection_id=i.connection_id AND ca.agent_id=target_agent AND ca.capability='channel'
 JOIN connections c ON c.id=ca.connection_id AND c.status='ready'
 LEFT JOIN chat_channel_identity_access a ON a.connection_id=i.connection_id AND a.agent_id=ca.agent_id AND a.identity_id=i.id
 WHERE i.id=sender AND (ca.access_mode='public' OR (ca.access_mode='private' AND (i.verified_at IS NOT NULL OR u.attested_at IS NOT NULL) AND a.allowed)));
$$;
CREATE TRIGGER identity_attestation_changed AFTER UPDATE OF attested_at ON chat_users
 FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_configuration');
