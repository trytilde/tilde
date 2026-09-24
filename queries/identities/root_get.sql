--: Record()
--! run (id) : Record
SELECT id,created_at FROM root_identities WHERE id=:id;
