UPDATE chat_inputs SET invocation_id=$2 WHERE invocation_id=$1 AND NOT accepted;
