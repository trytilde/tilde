--! run (id, thread, participant, text, at)
-- A message a local harness reported, at the time it was said. Its id is derived from the
-- harness's own, so a resent event inserts nothing.
INSERT INTO chat_messages(id, thread_id, participant_id, text, status, created_at)
VALUES (:id, :thread, :participant, :text, 'complete', :at)
ON CONFLICT (id) DO NOTHING;
