-- Message snapshots repeat their immutable links on edits and queue removal.
--! targets (message, thread, participant)
INSERT INTO chat_message_targets(message_id,participant_id)
SELECT :message,id FROM chat_participants WHERE thread_id=:thread AND id=:participant
ON CONFLICT(message_id,participant_id) DO NOTHING;
--! attachment (thread, message, attachment)
INSERT INTO chat_message_attachments(thread_id,message_id,attachment_id) VALUES(:thread,:message,:attachment)
ON CONFLICT(message_id,attachment_id) DO NOTHING;
