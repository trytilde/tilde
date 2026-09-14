INSERT INTO chat_message_dispatch(message_id,agent_id) VALUES($1,$2) ON CONFLICT DO NOTHING;
