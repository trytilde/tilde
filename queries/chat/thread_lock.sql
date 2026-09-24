--: Record()

--! run (p1) : Record
-- Serialize thread mutations without blocking foreign-key checks from the audit flusher.
SELECT id FROM chat_threads WHERE id=:p1 FOR NO KEY UPDATE;
