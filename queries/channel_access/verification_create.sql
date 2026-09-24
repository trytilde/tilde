--! run (p1, p2, p3, p4, p5)
INSERT INTO chat_identity_verifications(id,connection_id,agent_id,identity_id,token_hash,status,expires_at) VALUES(:p1,:p2,:p3,:p4,:p5,'pending',NOW()+INTERVAL '10 minutes');
