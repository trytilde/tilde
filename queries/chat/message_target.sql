INSERT INTO chat_message_targets(message_id,participant_id) SELECT $1,id FROM chat_participants WHERE thread_id=$2 AND id=$3 AND active;
