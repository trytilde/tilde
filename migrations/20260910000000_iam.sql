ALTER TABLE agents ADD COLUMN capabilities JSONB NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(capabilities) = 'object');
CREATE TABLE iam_signing_key (id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK(id), sealed BYTEA NOT NULL);
CREATE TABLE iam_users (
    id UUID PRIMARY KEY, issuer TEXT NOT NULL, subject TEXT NOT NULL,
    UNIQUE(issuer, subject)
);
CREATE TABLE iam_sessions (
    token_hash BYTEA PRIMARY KEY, user_id UUID NOT NULL REFERENCES iam_users(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX iam_sessions_expiry ON iam_sessions(expires_at);
CREATE TABLE iam_oidc_states (
    id UUID PRIMARY KEY, state_hash BYTEA NOT NULL UNIQUE, nonce TEXT NOT NULL,
    browser_hash BYTEA NOT NULL, verifier BYTEA NOT NULL, expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX iam_oidc_states_expiry ON iam_oidc_states(expires_at);
ALTER TABLE chat_invocations DROP COLUMN capability_hash;
