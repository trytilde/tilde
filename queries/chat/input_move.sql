--! run (p1, p2)
UPDATE chat_inputs SET invocation_id=:p2 WHERE invocation_id=:p1 AND NOT accepted;
