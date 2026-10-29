--! run (p1, p2?, p3?, p4?, p5?, p6?, p7?, p10, p8, p9?)
-- A person's edit: absent fields keep their value, and the memory becomes theirs. p8 clears the
-- time it holds; otherwise a new p6 (with p7 or ongoing p10) replaces it as memory_insert builds it.
UPDATE memories SET kind=COALESCE(:p2,kind),path=COALESCE(:p3,path),title=COALESCE(:p4,title),content=COALESCE(:p5,content),
  valid_during=CASE WHEN :p8 THEN NULL
       WHEN :p6::TIMESTAMPTZ IS NULL THEN valid_during
       WHEN :p10::BOOLEAN THEN tstzrange(:p6,NULL,'[)')
       WHEN :p7::TIMESTAMPTZ IS NULL OR :p7=:p6 THEN tstzrange(:p6,:p6,'[]')
       ELSE tstzrange(:p6,:p7,'[)') END,
  importance=COALESCE(:p9,importance),origin='owner',updated_at=NOW()
WHERE id=:p1 AND deleted_at IS NULL;
