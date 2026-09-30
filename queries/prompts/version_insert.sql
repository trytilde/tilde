--: Record()

--! run (p1, p2, p3, p4, p5, p6, p7, p8, p9) : Record
INSERT INTO prompt_versions(id,prompt_id,number,hash,template,config,variables,format,origin,deployment_id)
SELECT :p1,:p2,COALESCE(MAX(number),0)+1,:p3,:p4,:p5,:p6,:p7,:p8,:p9 FROM prompt_versions WHERE prompt_id=:p2
RETURNING id;
