DELETE FROM chat_typing WHERE thread_id=$1 AND participant_id=$2 AND expires_at<=NOW();
