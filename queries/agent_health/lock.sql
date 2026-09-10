-- One poller per database; rollback releases this lock even during cancellation.
SELECT pg_try_advisory_xact_lock(6073477207316124756) AS "acquired!";
