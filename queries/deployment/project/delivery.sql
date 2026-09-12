INSERT INTO chat_message_deliveries(message_id,connection_id,destination,external_message_id,status) VALUES($1,$2,$3,$4,$5) ON CONFLICT(message_id) DO UPDATE SET status=EXCLUDED.status;
