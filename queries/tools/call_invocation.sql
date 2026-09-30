--: Record()

--! run (p1) : Record
SELECT status FROM chat_invocations WHERE id=:p1;
