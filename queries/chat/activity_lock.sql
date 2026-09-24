--: Record()

--! run (p1) : Record
-- Separate from the mutation: readers after this lock see the preceding committed write.
SELECT activity_sequence + 1 AS sequence, NOW() AS at
FROM chat_threads WHERE id = :p1 FOR NO KEY UPDATE;
