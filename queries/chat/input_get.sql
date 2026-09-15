SELECT text,history_through_message_id FROM chat_inputs WHERE origin_invocation_id=$1 AND id=$2;
