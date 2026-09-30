--: Record(input_per_m_micros?, output_per_m_micros?, cached_input_per_m_micros?, cache_write_per_m_micros?, image_micros?, character_per_m_micros?, second_micros?)
-- A model named without its provider, as spans usually are: the first provider that lists it.
--! run (p1) : Record
SELECT provider_id,model,input_per_m_micros,output_per_m_micros,cached_input_per_m_micros,cache_write_per_m_micros,image_micros,character_per_m_micros,second_micros,source FROM inference_prices WHERE model=:p1 ORDER BY provider_id LIMIT 1;
