-- Flatten the catalog without silently changing credentials pinned to older definitions.
DO $$ BEGIN
 IF EXISTS (
  SELECT 1 FROM connections c
  WHERE c.provider_revision <> (SELECT max(p.revision) FROM connection_providers p WHERE p.provider_id=c.provider_id)
 ) THEN RAISE EXCEPTION 'Consolidate connections onto the current provider definition before removing provider revisions'; END IF;
END $$;
DELETE FROM connection_type_fields f WHERE provider_revision < (SELECT max(revision) FROM connection_providers p WHERE p.provider_id=f.provider_id);
DELETE FROM connection_oauth_parameters f WHERE provider_revision < (SELECT max(revision) FROM connection_providers p WHERE p.provider_id=f.provider_id);
DELETE FROM connection_oauth_result_fields f WHERE provider_revision < (SELECT max(revision) FROM connection_providers p WHERE p.provider_id=f.provider_id);
DELETE FROM connection_types t WHERE provider_revision < (SELECT max(revision) FROM connection_providers p WHERE p.provider_id=t.provider_id);
DELETE FROM connection_providers p WHERE revision < (SELECT max(q.revision) FROM connection_providers q WHERE q.provider_id=p.provider_id);

ALTER TABLE connection_providers ADD COLUMN kind TEXT;
ALTER TABLE connection_providers ADD COLUMN remote_authorization_id UUID;
UPDATE connection_providers SET kind=CASE WHEN built_in THEN 'built_in' WHEN remote_endpoint IS NOT NULL THEN 'remote' ELSE 'configured' END;
-- Retain the original cryptographic binding as an opaque credential identity. No re-encryption needed.
UPDATE connection_providers SET remote_authorization_id=encode(substring(sha256(convert_to(provider_id||':'||revision::text,'UTF8')) FROM 1 FOR 16),'hex')::uuid WHERE remote_authorization IS NOT NULL;
ALTER TABLE connection_providers ALTER COLUMN kind SET NOT NULL;
ALTER TABLE connection_providers ADD CHECK (kind IN ('built_in','configured','remote'));
ALTER TABLE connection_providers DROP CONSTRAINT remote_provider_configuration;
ALTER TABLE connection_providers ADD CONSTRAINT remote_provider_configuration CHECK (
 (kind='remote' AND remote_endpoint IS NOT NULL AND remote_ui_url IS NOT NULL AND remote_authorization IS NOT NULL AND remote_authorization_id IS NOT NULL) OR
 (kind<>'remote' AND remote_endpoint IS NULL AND remote_ui_url IS NULL AND remote_authorization IS NULL AND remote_authorization_id IS NULL)
);
DO $$ DECLARE item RECORD; BEGIN
 FOR item IN SELECT conrelid::regclass AS relation,conname FROM pg_constraint
 WHERE contype='f' AND conrelid=ANY(ARRAY['connection_types'::regclass,'connection_type_fields'::regclass,'connection_oauth_parameters'::regclass,'connection_oauth_result_fields'::regclass,'connections'::regclass])
 LOOP EXECUTE format('ALTER TABLE %s DROP CONSTRAINT %I',item.relation,item.conname); END LOOP;
END $$;
ALTER TABLE connection_providers DROP CONSTRAINT connection_providers_pkey;
ALTER TABLE connection_types DROP CONSTRAINT connection_types_pkey;
ALTER TABLE connection_type_fields DROP CONSTRAINT connection_type_fields_pkey;
ALTER TABLE connection_oauth_parameters DROP CONSTRAINT connection_oauth_parameters_pkey;
ALTER TABLE connection_oauth_result_fields DROP CONSTRAINT connection_oauth_result_fields_pkey;
ALTER TABLE connection_providers DROP COLUMN revision, DROP COLUMN built_in;
ALTER TABLE connection_types DROP COLUMN provider_revision;
ALTER TABLE connection_type_fields DROP COLUMN provider_revision;
ALTER TABLE connection_oauth_parameters DROP COLUMN provider_revision;
ALTER TABLE connection_oauth_result_fields DROP COLUMN provider_revision;
ALTER TABLE connections DROP COLUMN provider_revision;
ALTER TABLE connection_providers ADD PRIMARY KEY(provider_id);
ALTER TABLE connection_types ADD PRIMARY KEY(provider_id,type_id), ADD FOREIGN KEY(provider_id) REFERENCES connection_providers(provider_id);
ALTER TABLE connection_type_fields ADD PRIMARY KEY(provider_id,type_id,field_key), ADD FOREIGN KEY(provider_id,type_id) REFERENCES connection_types(provider_id,type_id);
ALTER TABLE connection_oauth_parameters ADD PRIMARY KEY(provider_id,type_id,phase,name), ADD FOREIGN KEY(provider_id,type_id) REFERENCES connection_types(provider_id,type_id);
ALTER TABLE connection_oauth_result_fields ADD PRIMARY KEY(provider_id,type_id,field_key), ADD FOREIGN KEY(provider_id,type_id) REFERENCES connection_types(provider_id,type_id);
ALTER TABLE connections ADD FOREIGN KEY(provider_id,type_id) REFERENCES connection_types(provider_id,type_id);
