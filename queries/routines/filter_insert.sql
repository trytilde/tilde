--! run (routine, position, path, allowed)
INSERT INTO routine_filters(routine_id,position,path,allowed) VALUES(:routine,:position,:path,:allowed::TEXT[]);
