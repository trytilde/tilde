UPDATE agents SET deployment_mode=$2,endpoint_url=$3,generation=generation+CASE WHEN deployment_mode<>$2 THEN 1 ELSE 0 END,updated_at=NOW() WHERE id=$1;
