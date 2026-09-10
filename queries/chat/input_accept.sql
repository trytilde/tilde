UPDATE chat_inputs SET accepted=TRUE WHERE invocation_id=$1 AND id=$2;
