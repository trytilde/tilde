INSERT INTO chat_inputs(invocation_id,id,text) VALUES($1,$2,$3) ON CONFLICT(invocation_id,id) DO NOTHING;
