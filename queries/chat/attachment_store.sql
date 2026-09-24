--! run (p1, p2, p3, p4)
UPDATE chat_attachments SET content=:p2,size_bytes=:p3,sha256=:p4 WHERE id=:p1;
