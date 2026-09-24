--: Record()

--! run (p1) : Record
SELECT EXISTS(SELECT 1 FROM chat_invocations WHERE agent_id=:p1 AND status IN ('pending','running')) AS active;
