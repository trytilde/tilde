ALTER TABLE connection_providers ADD COLUMN remote_endpoint TEXT;
ALTER TABLE connection_providers ADD COLUMN remote_ui_url TEXT;
ALTER TABLE connection_providers ADD COLUMN remote_authorization BYTEA;
ALTER TABLE connection_providers ADD CONSTRAINT remote_provider_configuration CHECK (
 (remote_endpoint IS NULL AND remote_ui_url IS NULL AND remote_authorization IS NULL) OR
 (remote_endpoint IS NOT NULL AND remote_ui_url IS NOT NULL AND remote_authorization IS NOT NULL)
);
ALTER TABLE connection_types DROP CONSTRAINT connection_types_driver_check;
ALTER TABLE connection_types ADD CHECK (driver IN ('static','oauth_code','oauth_client_credentials','oauth_jwt_bearer','catalog','remote'));
ALTER TABLE connection_setups ADD COLUMN provider_redirect_url TEXT;
CREATE TABLE connection_setup_drafts (
 setup_id UUID NOT NULL REFERENCES connection_setups(id) ON DELETE CASCADE,
 field_key TEXT NOT NULL,
 encrypted_value BYTEA NOT NULL,
 PRIMARY KEY(setup_id,field_key)
);
