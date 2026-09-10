UPDATE chat_inputs SET accepted=FALSE WHERE invocation_id=$1 AND id=$2;
