--! run (p1, p2, p3, p4, p5, p6, p7, p8, p9, p10, p11, p12, p13, p14, p15, p16, p17, p18, p19, p20, p21, p22, p23)
INSERT INTO inference_requests(id,created_at,agent_id,connection_id,invocation_id,thread_id,run_id,participant_id,provider_id,kind,path,model,status,latency_ms,first_byte_ms,request_bytes,response_bytes,input_tokens,output_tokens,cached_input_tokens,cache_write_tokens,units,usage,cost_micros)
SELECT u.id,u.created_at,u.agent_id,u.connection_id,u.invocation_id,u.thread_id,u.run_id,u.participant_id,u.provider_id,u.kind,u.path,NULLIF(u.model,''),u.status,u.latency_ms,NULLIF(u.first_byte_ms,-1),u.request_bytes,u.response_bytes,NULLIF(u.input_tokens,-1),NULLIF(u.output_tokens,-1),NULLIF(u.cached_input_tokens,-1),NULLIF(u.cache_write_tokens,-1),NULLIF(u.units,-1),u.usage,
 -- Base tier only: the response does not say which tier served it. Cached reads are a subset of
 -- input for OpenAI-style usage and additive for Anthropic and Bedrock, where cache writes are
 -- reported separately and billed at a premium. Missing cache rates fall back to the input rate.
 -- Image and speech models bill by whichever unit the sheet prices: per image or per character
 -- from the request, or seconds of audio from a transcription response, when that rate exists;
 -- otherwise by the tokens the response reported.
 CASE
  WHEN p.provider_id IS NULL THEN NULL
  WHEN u.kind='image' AND p.image_micros IS NOT NULL AND u.units>=0 THEN u.units*p.image_micros
  WHEN u.kind='speech' AND p.character_per_m_micros IS NOT NULL AND u.units>=0 THEN ROUND(u.units::NUMERIC*p.character_per_m_micros/1000000)::BIGINT
  WHEN u.kind='transcription' AND p.second_micros IS NOT NULL AND u.units>=0 THEN u.units*p.second_micros
  WHEN u.input_tokens<0 THEN NULL
  ELSE ROUND((
   (CASE WHEN u.provider_id IN ('anthropic','bedrock') THEN u.input_tokens ELSE u.input_tokens-GREATEST(u.cached_input_tokens,0) END)::NUMERIC*COALESCE(p.input_per_m_micros,0)
   + GREATEST(u.cached_input_tokens,0)::NUMERIC*COALESCE(p.cached_input_per_m_micros,p.input_per_m_micros,0)
   + GREATEST(u.cache_write_tokens,0)::NUMERIC*COALESCE(p.cache_write_per_m_micros,p.input_per_m_micros,0)
   + GREATEST(u.output_tokens,0)::NUMERIC*COALESCE(p.output_per_m_micros,0)
  )/1000000)::BIGINT END
FROM UNNEST(:p1::UUID[],:p2::TIMESTAMPTZ[],:p3::UUID[],:p4::UUID[],:p5::UUID[],:p6::UUID[],:p7::UUID[],:p8::UUID[],:p9::TEXT[],:p10::TEXT[],:p11::TEXT[],:p12::TEXT[],:p13::INTEGER[],:p14::INTEGER[],:p15::INTEGER[],:p16::BIGINT[],:p17::BIGINT[],:p18::BIGINT[],:p19::BIGINT[],:p20::BIGINT[],:p21::BIGINT[],:p22::BIGINT[],:p23::TEXT[])
 AS u(id,created_at,agent_id,connection_id,invocation_id,thread_id,run_id,participant_id,provider_id,kind,path,model,status,latency_ms,first_byte_ms,request_bytes,response_bytes,input_tokens,output_tokens,cached_input_tokens,cache_write_tokens,units,usage)
 LEFT JOIN inference_prices p ON p.provider_id=u.provider_id AND p.model=u.model
ON CONFLICT (id) DO NOTHING;
