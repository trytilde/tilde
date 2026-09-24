--! run (p1, p2, p3, p4, p5)
INSERT INTO chat_message_deliveries(message_id,connection_id,destination,external_message_id,status) VALUES(:p1,:p2,:p3,:p4,:p5) ON CONFLICT(message_id) DO UPDATE SET status=EXCLUDED.status;
