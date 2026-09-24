--! run (id, root_id?)
UPDATE chat_users SET root_identity_id=:root_id WHERE id=:id;
