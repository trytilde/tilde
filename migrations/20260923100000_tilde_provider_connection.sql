-- The built-in Tilde chat provider becomes a catalog provider. Every agent owns one
-- system-managed `tilde/application` connection whose sealed `api_key` value replaces
-- tilde_chat_keys; asserted identities live in chat_channel_identities and the access
-- policy in connection_agents like every other channel. The provider row is inserted
-- here (startup reconciles the full definition) so agent creation never depends on
-- catalog seeding.
INSERT INTO connection_providers(provider_id,name,kind,categories,icon_url)
VALUES('tilde','Tilde','built_in',ARRAY['chat'],'/tilde-mark.svg') ON CONFLICT(provider_id) DO NOTHING;
INSERT INTO connection_types(provider_id,type_id,name,driver,channel_capable,credential_schema)
VALUES('tilde','application','Application','static',TRUE,
 '{"type":"object","additionalProperties":false,"properties":{"api_key":{"type":"string","title":"API key","minLength":1,"writeOnly":true}},"required":["api_key"]}'::jsonb)
ON CONFLICT(provider_id,type_id) DO NOTHING;
DROP TABLE IF EXISTS tilde_chat_access;
DROP TABLE tilde_chat_identities;
DROP TABLE tilde_chat_keys;
