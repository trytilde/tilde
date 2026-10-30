--! run : (scan, ef)
-- For the current transaction: keep scanning the HNSW index until filtered results fill the
-- candidate list, and search at least that many neighbours.
SELECT set_config('hnsw.iterative_scan','relaxed_order',TRUE) AS scan, set_config('hnsw.ef_search','100',TRUE) AS ef;
