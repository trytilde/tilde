-- One migration history for the whole application. Domains do not own separate runners.
CREATE TABLE agents (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 200),
    endpoint_url TEXT,
    webhook_signing_key BYTEA NOT NULL CHECK (octet_length(webhook_signing_key) >= 46),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX agents_created ON agents(created_at DESC, id DESC);

-- One immutable data key for this installation, wrapped by the selected backend.
-- A changed seed/backend must never silently overwrite this record.
CREATE TABLE encryption_keys (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    id UUID NOT NULL UNIQUE,
    backend TEXT NOT NULL CHECK (backend IN ('seed', 'aws_kms')),
    wrapping_key_id TEXT NOT NULL,
    wrap_nonce BYTEA,
    wrapped_key BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK ((backend = 'seed' AND wrap_nonce IS NOT NULL AND octet_length(wrap_nonce) = 12) OR (backend = 'aws_kms' AND wrap_nonce IS NULL))
);
