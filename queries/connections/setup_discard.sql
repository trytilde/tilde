DELETE FROM connection_setup_values WHERE setup_id IN (SELECT id FROM connection_setups WHERE connection_id=$1);
