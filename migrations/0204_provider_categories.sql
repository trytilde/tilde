ALTER TABLE connection_providers ADD COLUMN categories TEXT[] NOT NULL DEFAULT '{other}';
UPDATE connection_providers SET categories=CASE provider_id
 WHEN 'github' THEN ARRAY['developer_tools']
 WHEN 'agentmail' THEN ARRAY['email']
 WHEN 'slack' THEN ARRAY['chat'] WHEN 'linq' THEN ARRAY['chat']
 WHEN 'whatsapp' THEN ARRAY['chat'] WHEN 'telnyx' THEN ARRAY['chat']
 ELSE ARRAY['other'] END;
ALTER TABLE connection_types ADD CONSTRAINT credential_source_configuration CHECK (
 (driver IN ('oauth_code','oauth_client_credentials','oauth_jwt_bearer') AND token_url IS NOT NULL) OR
 (driver NOT IN ('oauth_code','oauth_client_credentials','oauth_jwt_bearer') AND token_url IS NULL)
);
