--: Record()

--! run (p1) : Record
SELECT expires_at=created_at+interval '10 minutes' AS ten_minutes FROM connection_setups WHERE id=:p1;
