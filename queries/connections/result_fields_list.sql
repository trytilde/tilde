--: Record()

--! run (p1, p2) : Record
SELECT field_key,json_pointer,required FROM connection_oauth_result_fields WHERE provider_id=:p1 AND type_id=:p2 ORDER BY field_key;
