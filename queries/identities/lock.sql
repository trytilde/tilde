--: Record(root_identity_id?)
--! run (id) : Record
SELECT id,root_identity_id FROM chat_users WHERE id=:id FOR UPDATE;
