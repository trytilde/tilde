-- Sandbox egress: a blueprint's grants to reach a connection's upstream through the gateway with
-- the connection's credential, which never enters the sandbox. A connection type offers it when
-- its provider declares egress protocols (built-in catalog code).
ALTER TABLE connection_types ADD COLUMN egress_capable BOOLEAN NOT NULL DEFAULT false;
-- A grant is off until enabled. `config` is typed: validated against the provider's egress
-- config schema at `config_version` (approved JSONB) and only narrows the provider's rules.
-- Connections in use by a grant cannot be deleted.
CREATE TABLE sandbox_blueprint_egress (
    blueprint_id UUID NOT NULL REFERENCES sandbox_blueprints(id) ON DELETE CASCADE,
    slug TEXT NOT NULL CHECK (slug ~ '^[a-z][a-z0-9_]{0,31}$'),
    connection_id UUID NOT NULL REFERENCES connections(id) ON DELETE RESTRICT,
    config JSONB NOT NULL,
    config_version INTEGER NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (blueprint_id, slug),
    UNIQUE (blueprint_id, connection_id)
);
CREATE INDEX sandbox_blueprint_egress_connection ON sandbox_blueprint_egress(connection_id);
