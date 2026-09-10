SELECT thread_id,participant_id FROM chat_typing WHERE expires_at<=NOW() ORDER BY thread_id,participant_id LIMIT 100;
