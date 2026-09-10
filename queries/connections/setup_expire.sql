UPDATE connection_setups SET step='failed',error_code='setup_expired',claimed_at=NULL,updated_at=now() WHERE connection_id=$1 AND expires_at<=now() AND step NOT IN ('complete','failed','cancelled');
