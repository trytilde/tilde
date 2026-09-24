--: Record()

--! run (p1) : Record
SELECT EXISTS(SELECT 1 FROM chat_threads WHERE id=:p1) AS exists;
