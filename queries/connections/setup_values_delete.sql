WITH removed_draft AS (DELETE FROM connection_setup_drafts WHERE setup_id=$1)
DELETE FROM connection_setup_values WHERE setup_id=$1;
