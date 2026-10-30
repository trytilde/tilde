--! run (p1, p2, p3?, p4, p5) : (path, count)
-- The paths directly below p2 in banks p1, each with how many current memories it holds (itself
-- and below), with recall_text's person filter; at most p5.
SELECT CASE WHEN :p2='' THEN split_part(m.path,'/',1)
            ELSE :p2||'/'||split_part(substr(m.path,length(:p2)+2),'/',1) END AS path,
  COUNT(*) AS count
FROM current_memories m
WHERE m.bank_id=ANY(:p1::UUID[])
  AND ((:p2='' AND m.path<>'') OR starts_with(m.path,:p2||'/'))
  AND (:p4 OR m.scope='shared' OR m.identity_id IS NULL OR m.identity_id=:p3::UUID)
GROUP BY 1 ORDER BY 1 LIMIT :p5;
