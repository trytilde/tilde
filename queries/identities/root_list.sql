--: Record()
--! run (after?, limit_count) : Record
SELECT id,created_at FROM root_identities WHERE (:after::UUID IS NULL OR id>:after) ORDER BY id LIMIT :limit_count;
