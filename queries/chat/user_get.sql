--: Record()

--! run (p1) : Record
SELECT id,name FROM chat_users WHERE id=:p1;
