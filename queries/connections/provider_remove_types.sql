DELETE FROM connection_types WHERE provider_id=$1 AND NOT(type_id=ANY($2));
