DELETE FROM chat_identity_verifications WHERE expires_at < NOW()-INTERVAL '1 day';
