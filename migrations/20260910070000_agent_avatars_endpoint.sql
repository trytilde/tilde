ALTER TABLE agents ADD COLUMN avatar_seed UUID NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE agents ADD COLUMN avatar_key TEXT;
-- Enforce the requirement for every new or updated row without inventing endpoints
-- for historical registrations. Existing missing endpoints must be configured.
ALTER TABLE agents ADD CONSTRAINT agents_endpoint_required
    CHECK (deleted_at IS NOT NULL OR (endpoint_url IS NOT NULL AND length(trim(endpoint_url)) > 0)) NOT VALID;
