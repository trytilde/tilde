--: Record()

--! run (routines) : Record
SELECT routine_id,path,allowed FROM routine_filters WHERE routine_id=ANY(:routines::UUID[]) ORDER BY routine_id,position;
