SELECT field_key,json_pointer,required FROM connection_oauth_result_fields WHERE provider_id=$1 AND type_id=$2 ORDER BY field_key;
