--! run (p1)
WITH removed_draft AS (DELETE FROM connection_setup_drafts WHERE setup_id=:p1)
DELETE FROM connection_setup_values WHERE setup_id=:p1;
