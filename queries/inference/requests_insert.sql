-- Cost is computed by crate::pricing before the insert, the one place prices are applied.
--! run (p1, p2, p3, p4, p5, p6, p7, p8, p9, p10, p11, p12, p13, p14, p15, p16, p17, p18, p19, p20, p21, p22, p23, p24)
INSERT INTO inference_requests(id,created_at,agent_id,connection_id,invocation_id,thread_id,run_id,participant_id,provider_id,kind,path,model,status,latency_ms,first_byte_ms,request_bytes,response_bytes,input_tokens,output_tokens,cached_input_tokens,cache_write_tokens,units,usage,cost_micros)
SELECT u.id,u.created_at,u.agent_id,u.connection_id,u.invocation_id,u.thread_id,u.run_id,u.participant_id,u.provider_id,u.kind,u.path,NULLIF(u.model,''),u.status,u.latency_ms,NULLIF(u.first_byte_ms,-1),u.request_bytes,u.response_bytes,NULLIF(u.input_tokens,-1),NULLIF(u.output_tokens,-1),NULLIF(u.cached_input_tokens,-1),NULLIF(u.cache_write_tokens,-1),NULLIF(u.units,-1),u.usage,
 NULLIF(u.cost_micros,-1)
FROM UNNEST(:p1::UUID[],:p2::TIMESTAMPTZ[],:p3::UUID[],:p4::UUID[],:p5::UUID[],:p6::UUID[],:p7::UUID[],:p8::UUID[],:p9::TEXT[],:p10::TEXT[],:p11::TEXT[],:p12::TEXT[],:p13::INTEGER[],:p14::INTEGER[],:p15::INTEGER[],:p16::BIGINT[],:p17::BIGINT[],:p18::BIGINT[],:p19::BIGINT[],:p20::BIGINT[],:p21::BIGINT[],:p22::BIGINT[],:p23::TEXT[],:p24::BIGINT[])
 AS u(id,created_at,agent_id,connection_id,invocation_id,thread_id,run_id,participant_id,provider_id,kind,path,model,status,latency_ms,first_byte_ms,request_bytes,response_bytes,input_tokens,output_tokens,cached_input_tokens,cache_write_tokens,units,usage,cost_micros)
ON CONFLICT (id) DO NOTHING;
