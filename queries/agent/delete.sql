UPDATE agents SET deleted_at=NOW(), endpoint_url=NULL, webhook_signing_key=''::bytea, capabilities='{}'::jsonb, updated_at=NOW() WHERE id=$1 AND paused AND deleted_at IS NULL;
