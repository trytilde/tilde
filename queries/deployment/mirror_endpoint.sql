UPDATE agents SET endpoint_url=$2,updated_at=NOW() WHERE id=$1 AND deployment_mode='gateway' AND endpoint_url IS DISTINCT FROM $2;
