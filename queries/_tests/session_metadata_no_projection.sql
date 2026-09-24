SELECT to_regclass('chat_session_projection_pending') IS NULL AS no_queue,
 to_regclass('chat_session_projection_version') IS NULL AS no_sequence,
 NOT EXISTS(SELECT 1 FROM pg_trigger WHERE tgname LIKE 'chat_session_projection_%') AS no_triggers;
