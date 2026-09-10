WITH parameters AS (DELETE FROM connection_oauth_parameters WHERE provider_id=$1)
DELETE FROM connection_oauth_result_fields WHERE provider_id=$1;
