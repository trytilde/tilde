--: Record(suspend_at?)

--! run (p1) : Record
SELECT suspend_at FROM chat_invocations WHERE id = :p1;
