SELECT id FROM agents WHERE id=$1 AND deleted_at IS NULL FOR SHARE;
