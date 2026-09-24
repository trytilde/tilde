--! run (p1, p2, p3, p4, p5, p6, p7, p8, p9)
INSERT INTO inference_prices(provider_id,model,input_per_m_micros,output_per_m_micros,cached_input_per_m_micros,cache_write_per_m_micros,image_micros,character_per_m_micros,second_micros,source,updated_at)
SELECT u.provider_id,u.model,NULLIF(u.input_per_m,-1),NULLIF(u.output_per_m,-1),NULLIF(u.cached_per_m,-1),NULLIF(u.write_per_m,-1),NULLIF(u.image,-1),NULLIF(u.character_per_m,-1),NULLIF(u.second,-1),'litellm',NOW()
FROM UNNEST(:p1::TEXT[],:p2::TEXT[],:p3::BIGINT[],:p4::BIGINT[],:p5::BIGINT[],:p6::BIGINT[],:p7::BIGINT[],:p8::BIGINT[],:p9::BIGINT[]) AS u(provider_id,model,input_per_m,output_per_m,cached_per_m,write_per_m,image,character_per_m,second)
ON CONFLICT (provider_id,model) DO UPDATE SET input_per_m_micros=excluded.input_per_m_micros,output_per_m_micros=excluded.output_per_m_micros,cached_input_per_m_micros=excluded.cached_input_per_m_micros,cache_write_per_m_micros=excluded.cache_write_per_m_micros,image_micros=excluded.image_micros,character_per_m_micros=excluded.character_per_m_micros,second_micros=excluded.second_micros,updated_at=NOW()
WHERE inference_prices.source='litellm';
