--! run (p1, p2, p3, p4)
INSERT INTO chat_message_deliveries(message_id,connection_id,destination,external_message_id) VALUES(:p1,:p2,:p3,:p4);
