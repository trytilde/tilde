-- Asserted identities go with the agent's Tilde connection; their chat users and history stay.
--! run (p1)
DELETE FROM chat_channel_identities WHERE connection_id=:p1;
