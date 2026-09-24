--! run (p1, p2, p3)
INSERT INTO chat_message_targets(message_id,participant_id) SELECT :p1,id FROM chat_participants WHERE thread_id=:p2 AND id=:p3 AND active;
