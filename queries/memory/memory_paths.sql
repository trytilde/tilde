--! run (p1, p2) : ()
-- Every path in bank p1 holding current memories, with how many it holds exactly; at most p2.
SELECT m.path, COUNT(*) AS count
FROM current_memories m WHERE m.bank_id=:p1
GROUP BY m.path ORDER BY m.path LIMIT :p2;
