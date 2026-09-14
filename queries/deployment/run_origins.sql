SELECT id,source_identity_id,channel_origin FROM chat_runs WHERE thread_id=$1 AND agent_id=$2 AND status IN ('active','suspending','waiting');
