--: Record()

--! run (p1) : Record
SELECT
 (SELECT count(*) FROM connection_setups WHERE id=:p1) AS setups,
 (SELECT count(*) FROM connection_setup_drafts WHERE setup_id=:p1) AS drafts,
 (SELECT count(*) FROM connection_setup_values WHERE setup_id=:p1) AS staged;
