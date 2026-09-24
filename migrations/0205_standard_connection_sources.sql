ALTER TABLE connection_types ADD COLUMN credential_schema JSONB;
-- Convert existing static field definitions once; JSON Schema becomes the only form contract.
UPDATE connection_types t SET credential_schema=jsonb_build_object(
 'type','object','additionalProperties',false,
 'properties',coalesce((SELECT jsonb_object_agg(f.field_key,jsonb_strip_nulls(jsonb_build_object(
   'type',CASE f.kind WHEN 'boolean' THEN 'boolean' WHEN 'integer' THEN 'integer' ELSE 'string' END,
   'title',f.label,
   'writeOnly',CASE WHEN f.kind IN ('secret','multiline_secret') THEN true END,
   'minLength',CASE WHEN f.required AND f.kind NOT IN ('boolean','integer') THEN 1 END,
   'format',CASE WHEN f.kind='url' THEN 'uri' END,
   'pattern',CASE WHEN f.kind='phone' THEN '^\+[0-9]{7,15}$' END,
   'contentMediaType',CASE WHEN f.kind='multiline_secret' THEN 'application/x-pem-file' END,
   'enum',CASE WHEN f.kind='choice' THEN to_jsonb(f.choices) END
 ))) FROM connection_type_fields f WHERE f.provider_id=t.provider_id AND f.type_id=t.type_id AND (t.driver='static' OR f.field_key NOT IN ('client_id','client_secret','issuer','private_key','subject'))),'{}'::jsonb),
 'required',coalesce((SELECT jsonb_agg(f.field_key ORDER BY f.position) FROM connection_type_fields f WHERE f.provider_id=t.provider_id AND f.type_id=t.type_id AND f.required AND (t.driver='static' OR f.field_key NOT IN ('client_id','client_secret','issuer','private_key','subject'))),'[]'::jsonb)
) WHERE driver IN ('static','oauth_code','oauth_client_credentials','oauth_jwt_bearer');
UPDATE connection_types SET credential_schema=NULL WHERE driver<>'static' AND credential_schema->'properties'='{}'::jsonb;
ALTER TABLE connection_types DROP CONSTRAINT connection_types_driver_check;
UPDATE connection_types SET driver='custom' WHERE driver IN ('catalog','remote');
ALTER TABLE connection_types ADD CHECK (driver IN ('static','oauth_code','oauth_client_credentials','oauth_jwt_bearer','custom'));
ALTER TABLE connection_types ADD CHECK ((driver='static' AND credential_schema IS NOT NULL) OR (driver IN ('oauth_code','oauth_client_credentials','oauth_jwt_bearer')) OR (driver='custom' AND credential_schema IS NULL));
DROP TABLE connection_type_fields;
-- Standard credential sources need neither a remotely hosted setup UI nor a setup-handler token.
ALTER TABLE connection_providers DROP CONSTRAINT remote_provider_configuration;
ALTER TABLE connection_providers ADD CONSTRAINT remote_provider_configuration CHECK (
 (kind='remote' AND remote_endpoint IS NOT NULL AND ((remote_authorization IS NULL AND remote_authorization_id IS NULL) OR (remote_authorization IS NOT NULL AND remote_authorization_id IS NOT NULL))) OR
 (kind<>'remote' AND remote_endpoint IS NULL AND remote_ui_url IS NULL AND remote_authorization IS NULL AND remote_authorization_id IS NULL)
);
