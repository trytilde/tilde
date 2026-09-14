INSERT INTO agent_deployments(id,agent_id,source,target,endpoint_url,target_reference,repository,commit_sha,external_id,label,token_hash)
VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) RETURNING id;
