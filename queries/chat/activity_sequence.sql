UPDATE chat_threads SET activity_sequence=activity_sequence+1 WHERE id=$1 RETURNING activity_sequence;
