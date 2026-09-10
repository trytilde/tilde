SELECT phase,name,value FROM connection_oauth_parameters WHERE provider_id=$1 AND type_id=$2 ORDER BY phase,name;
