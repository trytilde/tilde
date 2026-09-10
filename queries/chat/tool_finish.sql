UPDATE chat_tool_calls SET status=$2,output_json=$3,error=$4,updated_at=NOW() WHERE id=$1 AND status='running';
