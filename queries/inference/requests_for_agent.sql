--: Record(model?, first_byte_ms?, input_tokens?, output_tokens?, cached_input_tokens?, cache_write_tokens?, units?, cost_micros?)

--! run (p1) : Record
SELECT id,created_at,connection_id,invocation_id,thread_id,run_id,participant_id,provider_id,kind,path,model,status,latency_ms,first_byte_ms,request_bytes,response_bytes,input_tokens,output_tokens,cached_input_tokens,cache_write_tokens,units,usage,cost_micros FROM inference_requests WHERE agent_id=:p1 ORDER BY created_at DESC,id LIMIT 200;
