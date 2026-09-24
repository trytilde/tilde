-- Preserve the existing ledger when upgrading a registry that used SQLx.
DO $$ BEGIN
    IF to_regclass('_tilde_migrations') IS NULL AND to_regclass('_sqlx_migrations') IS NOT NULL THEN
        ALTER TABLE _sqlx_migrations RENAME TO _tilde_migrations;
    END IF;
END $$;
CREATE TABLE IF NOT EXISTS _tilde_migrations (
    version BIGINT PRIMARY KEY,
    description TEXT NOT NULL,
    installed_on TIMESTAMPTZ NOT NULL DEFAULT now(),
    success BOOLEAN NOT NULL,
    checksum BYTEA NOT NULL,
    execution_time BIGINT NOT NULL
);
