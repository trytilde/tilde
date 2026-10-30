--! run (p1, p2, p3, p4, p5, p6, p7, p8?, p9?, p10)
-- Inserts nothing when a revision already has the call id: the write is a retry. The thread is
-- kept only once the gateway has it.
INSERT INTO wiki_page_revisions(page_id,wiki_id,revision,operation,path,title,content,agent_id,thread_id,call_id)
VALUES(:p1,:p2,:p3,:p4,:p5,:p6,:p7,:p8,(SELECT id FROM chat_threads WHERE id=:p9),:p10)
ON CONFLICT(call_id) DO NOTHING;
