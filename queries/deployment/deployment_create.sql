--: Record()

--! run (p1, p2, p3, p4, p5?, p6?, p7?, p8?, p9, p10, p11?, p12?, p13?) : Record
INSERT INTO agent_deployments(id,agent_id,source,target,target_reference,repository,commit_sha,external_id,label,token_hash,commit_message,branch,commit_author)
VALUES(:p1,:p2,:p3,:p4,:p5,:p6,:p7,:p8,:p9,:p10,:p11,:p12,:p13) RETURNING id;
