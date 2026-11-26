--: Record(suspended_for?)

--! run (p1) : Record
-- Why an invocation was asked to suspend, and whether it saved its state before it ended.
SELECT suspended_for, checkpointed AS saved FROM chat_invocations WHERE id = :p1;
