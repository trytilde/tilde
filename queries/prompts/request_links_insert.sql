--! run (request_ids, version_ids)
INSERT INTO inference_request_prompts(request_id,version_id)
SELECT * FROM UNNEST(:request_ids::UUID[],:version_ids::UUID[])
ON CONFLICT DO NOTHING;
