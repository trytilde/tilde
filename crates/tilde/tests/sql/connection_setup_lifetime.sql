SELECT expires_at=created_at+interval '10 minutes' AS "ten_minutes!" FROM connection_setups WHERE id=$1;
