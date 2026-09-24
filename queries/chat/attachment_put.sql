--: Record()

--! run (p1, p2, p3, p4, p5, p6, p7) : Record
-- No-op updates let concurrent identical retries return the existing row atomically.
INSERT INTO chat_attachments(id, thread_id, filename, media_type, size_bytes, sha256, content)
VALUES(:p1, :p2, :p3, :p4, :p5, :p6, :p7)
ON CONFLICT(id) DO UPDATE SET id = chat_attachments.id
WHERE chat_attachments.thread_id = EXCLUDED.thread_id
  AND chat_attachments.sha256 = EXCLUDED.sha256
  AND chat_attachments.filename = EXCLUDED.filename
  AND chat_attachments.media_type = EXCLUDED.media_type
RETURNING filename, media_type, size_bytes, sha256;
