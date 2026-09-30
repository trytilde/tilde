--! run (p1)
-- A host's provider goes with it, and so does every instance: the host is what served them.
WITH provider AS (SELECT provider_id FROM connection_providers WHERE tool_host_id=:p1),
removed AS (DELETE FROM connections WHERE provider_id IN (SELECT provider_id FROM provider)),
types AS (DELETE FROM connection_types WHERE provider_id IN (SELECT provider_id FROM provider)),
parameters AS (DELETE FROM connection_oauth_parameters WHERE provider_id IN (SELECT provider_id FROM provider))
DELETE FROM connection_oauth_result_fields WHERE provider_id IN (SELECT provider_id FROM provider);
