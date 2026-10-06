--! run
-- Calls abandoned by a crashed waiter.
DELETE FROM sandbox_calls WHERE created_at < NOW() - INTERVAL '1 hour';
