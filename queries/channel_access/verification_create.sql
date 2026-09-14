INSERT INTO chat_identity_verifications(id,connection_id,agent_id,identity_id,token_hash,status,expires_at) VALUES($1,$2,$3,$4,$5,'pending',NOW()+INTERVAL '10 minutes');
