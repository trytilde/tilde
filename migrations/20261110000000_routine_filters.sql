-- A signal routine may narrow its signal with filters: each names a path in the signal's context
-- (as templates do) and the values it may hold. Every filter must match for the routine to fire;
-- a filter matches when the value at its path (or any element, for a list) equals one of its
-- values. Cron routines have none.
CREATE TABLE routine_filters (
 routine_id UUID NOT NULL REFERENCES routines(id) ON DELETE CASCADE,
 position INTEGER NOT NULL CHECK (position BETWEEN 0 AND 19),
 path TEXT NOT NULL CHECK (length(path) BETWEEN 1 AND 200),
 allowed TEXT[] NOT NULL CHECK (cardinality(allowed) BETWEEN 1 AND 50),
 PRIMARY KEY (routine_id, position)
);
