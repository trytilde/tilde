UPDATE connection_setups SET step='failed',error_code=$3,claimed_at=NULL,updated_at=now() WHERE id=$1 AND action_id=$2 AND step NOT IN ('complete','cancelled');
