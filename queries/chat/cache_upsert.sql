INSERT INTO chat_converted_messages(agent_id,message_id,representation) VALUES($1,$2,$3) ON CONFLICT(agent_id,message_id) DO UPDATE SET representation=EXCLUDED.representation;
