--: Record(input_per_m_micros?, output_per_m_micros?, cached_input_per_m_micros?, cache_write_per_m_micros?, image_micros?, character_per_m_micros?, second_micros?)

--! run (p1, p2) : Record
SELECT provider_id,model,input_per_m_micros,output_per_m_micros,cached_input_per_m_micros,cache_write_per_m_micros,image_micros,character_per_m_micros,second_micros,source FROM inference_prices WHERE provider_id=:p1 AND model=:p2;
