--: Record(suspended_for?)

--! run (p1) : Record
-- Why an invocation was asked to suspend, and whether it saved its state before it ended.
SELECT i.suspended_for, EXISTS(SELECT 1 FROM chat_run_checkpoints c WHERE c.run_id = i.run_id AND c.invocation_id = i.id) AS saved
FROM chat_invocations i WHERE i.id = :p1;
