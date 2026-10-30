--! run : (timeout)
-- For the current transaction: a pattern that would scan for too long is cancelled.
SELECT set_config('statement_timeout','3000',TRUE) AS timeout;
