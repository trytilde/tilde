UPDATE chat_tool_calls SET input_json=input_json || $2,updated_at=NOW() WHERE id=$1 AND status='running';
