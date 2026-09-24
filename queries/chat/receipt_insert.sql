--! run (p1, p2, p3?)
INSERT INTO chat_channel_receipts(connection_id,external_id,message_id) VALUES(:p1,:p2,:p3);
