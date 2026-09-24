--! run (p1, p2)
UPDATE chat_inputs SET accepted=TRUE WHERE invocation_id=:p1 AND id=:p2;
