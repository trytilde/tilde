--! run (p1)
WITH parameters AS (DELETE FROM connection_oauth_parameters WHERE provider_id=:p1)
DELETE FROM connection_oauth_result_fields WHERE provider_id=:p1;
