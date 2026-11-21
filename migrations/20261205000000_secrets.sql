-- Secrets: named values people set in the app (Connections → Secrets) that specs reference by
-- name, such as a sandbox blueprint's environment variable. Values are sealed by the encryption
-- crate and never read back through management.
CREATE TABLE secrets (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL CHECK (name ~ '^[A-Za-z_][A-Za-z0-9_]{0,127}$'),
    description TEXT NOT NULL DEFAULT '' CHECK (length(description) <= 500),
    encrypted_value BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX secrets_name ON secrets(name);
-- A blueprint variable is a sealed value of its own or a reference to a secret, read at launch,
-- so rotating the secret reaches new sandboxes without a deploy. A secret in use cannot go.
ALTER TABLE sandbox_blueprint_env
    ADD COLUMN secret_id UUID REFERENCES secrets(id) ON DELETE RESTRICT,
    ALTER COLUMN encrypted_value DROP NOT NULL,
    ADD CONSTRAINT sandbox_blueprint_env_value CHECK ((encrypted_value IS NULL) <> (secret_id IS NULL));
CREATE INDEX sandbox_blueprint_env_secret ON sandbox_blueprint_env(secret_id) WHERE secret_id IS NOT NULL;
