--! run (p1, p2, p3, p4, p5, p6, p7?, p8?, p15, p9, p10, p11?, p12?, p13?, p14?)
-- From p7: ongoing [p7,) when p15, the period [p7, p8) when p8 is later, otherwise the moment p7. With a
-- thread (p14), the message is the latest one a person sent there. A thread not yet projected
-- from a sidecar leaves both unset.
INSERT INTO memories(id,bank_id,kind,path,title,content,valid_during,importance,origin,identity_id,written_by_agent_id,supersedes_id,thread_id,message_id)
VALUES(:p1,:p2,:p3,:p4,:p5,:p6,
  CASE WHEN :p7::TIMESTAMPTZ IS NULL THEN NULL
       WHEN :p15::BOOLEAN THEN tstzrange(:p7,NULL,'[)')
       WHEN :p8::TIMESTAMPTZ IS NULL OR :p8=:p7 THEN tstzrange(:p7,:p7,'[]')
       ELSE tstzrange(:p7,:p8,'[)') END,
  :p9,:p10,:p11,:p12,:p13,
  (SELECT id FROM chat_threads WHERE id=:p14::UUID),
  (SELECT m.id FROM chat_messages m JOIN chat_participants p ON p.id=m.participant_id
   WHERE m.thread_id=:p14 AND p.user_id IS NOT NULL ORDER BY m.created_at DESC LIMIT 1));
