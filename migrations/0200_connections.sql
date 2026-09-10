-- Definitions are versioned data. Existing connections keep their selected revision.
CREATE TABLE connection_providers (
    provider_id TEXT NOT NULL,
    revision BIGINT NOT NULL CHECK (revision > 0),
    name TEXT NOT NULL,
    built_in BOOLEAN NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (provider_id, revision)
);
CREATE TABLE connection_types (
    provider_id TEXT NOT NULL,
    provider_revision BIGINT NOT NULL,
    type_id TEXT NOT NULL,
    name TEXT NOT NULL,
    driver TEXT NOT NULL CHECK (driver IN ('static','oauth_code','oauth_client_credentials','oauth_jwt_bearer','catalog')),
    channel_capable BOOLEAN NOT NULL DEFAULT false,
    authorization_url TEXT,
    token_url TEXT,
    client_auth TEXT NOT NULL DEFAULT 'body' CHECK (client_auth IN ('body','basic','none')),
    pkce BOOLEAN NOT NULL DEFAULT true,
    scopes TEXT[] NOT NULL DEFAULT '{}',
    scope_separator TEXT NOT NULL DEFAULT ' ',
    access_token_path TEXT NOT NULL DEFAULT '/access_token',
    refresh_token_path TEXT NOT NULL DEFAULT '/refresh_token',
    expires_in_path TEXT NOT NULL DEFAULT '/expires_in',
    scope_path TEXT NOT NULL DEFAULT '/scope',
    success_path TEXT,
    PRIMARY KEY (provider_id, provider_revision, type_id),
    FOREIGN KEY (provider_id, provider_revision) REFERENCES connection_providers(provider_id, revision)
);
CREATE TABLE connection_type_fields (
    provider_id TEXT NOT NULL,
    provider_revision BIGINT NOT NULL,
    type_id TEXT NOT NULL,
    field_key TEXT NOT NULL,
    label TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('text','secret','multiline_secret','url','phone','integer','boolean','choice')),
    required BOOLEAN NOT NULL,
    choices TEXT[] NOT NULL DEFAULT '{}',
    position INTEGER NOT NULL,
    PRIMARY KEY (provider_id, provider_revision, type_id, field_key),
    FOREIGN KEY (provider_id, provider_revision, type_id) REFERENCES connection_types(provider_id, provider_revision, type_id)
);
CREATE TABLE connection_oauth_parameters (
    provider_id TEXT NOT NULL,
    provider_revision BIGINT NOT NULL,
    type_id TEXT NOT NULL,
    phase TEXT NOT NULL CHECK (phase IN ('authorization','token')),
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (provider_id, provider_revision, type_id, phase, name),
    FOREIGN KEY (provider_id, provider_revision, type_id) REFERENCES connection_types(provider_id, provider_revision, type_id)
);
CREATE TABLE connections (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    provider_revision BIGINT NOT NULL,
    type_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('requires_action','ready','failed','disconnected')),
    account_label TEXT,
    token_expires_at TIMESTAMPTZ,
    credential_version BIGINT NOT NULL DEFAULT 0,
    refresh_claim_until TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (provider_id, provider_revision, type_id) REFERENCES connection_types(provider_id, provider_revision, type_id)
);
CREATE INDEX connections_page ON connections(created_at DESC,id DESC);
-- Every value, including custom field values, is an authenticated encrypted envelope.
CREATE TABLE connection_values (
    connection_id UUID NOT NULL REFERENCES connections(id) ON DELETE CASCADE,
    field_key TEXT NOT NULL,
    encrypted_value BYTEA NOT NULL,
    PRIMARY KEY (connection_id, field_key)
);
CREATE TABLE connection_setups (
    id UUID PRIMARY KEY,
    connection_id UUID NOT NULL REFERENCES connections(id) ON DELETE CASCADE,
    step TEXT NOT NULL CHECK (length(step) BETWEEN 1 AND 160),
    action_id UUID NOT NULL,
    connection_setup_token BYTEA NOT NULL,
    connection_setup_token_hash BYTEA NOT NULL,
    callback_token BYTEA NOT NULL,
    callback_hash BYTEA NOT NULL,
    error_code TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    claimed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX connection_setup_active ON connection_setups(connection_id)
    WHERE step NOT IN ('complete','cancelled','failed');
CREATE TABLE connection_setup_values (
    setup_id UUID NOT NULL REFERENCES connection_setups(id) ON DELETE CASCADE,
    field_key TEXT NOT NULL,
    encrypted_value BYTEA NOT NULL,
    PRIMARY KEY (setup_id, field_key)
);
-- Explicit mappings for provider-owned token response facts; these are not lifecycle metadata.
CREATE TABLE connection_oauth_result_fields (
    provider_id TEXT NOT NULL,
    provider_revision BIGINT NOT NULL,
    type_id TEXT NOT NULL,
    field_key TEXT NOT NULL,
    json_pointer TEXT NOT NULL,
    required BOOLEAN NOT NULL,
    PRIMARY KEY (provider_id,provider_revision,type_id,field_key),
    FOREIGN KEY (provider_id,provider_revision,type_id) REFERENCES connection_types(provider_id,provider_revision,type_id)
);
