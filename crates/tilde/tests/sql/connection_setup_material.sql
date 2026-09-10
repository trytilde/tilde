SELECT
 (SELECT count(*) FROM connection_setups WHERE id=$1) AS "setups!",
 (SELECT count(*) FROM connection_setup_drafts WHERE setup_id=$1) AS "drafts!",
 (SELECT count(*) FROM connection_setup_values WHERE setup_id=$1) AS "staged!";
