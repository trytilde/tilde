--: MessageId()
--: Sequence()
--: Agent()
--: Run()
--: ReadState()
--: Session()
--: Queue()

--! attest_identity (user_id)
UPDATE chat_users SET attested_at=NOW() WHERE id=:user_id AND attested_at IS NULL;
--! sessions (agent, user_id?, query, after?, limit) : Session
SELECT t.id,t.title,t.primary_agent_id,
 COALESCE((SELECT m.text FROM chat_messages m WHERE m.thread_id=t.id ORDER BY m.created_at DESC,m.id DESC LIMIT 1),'') AS preview,
 (COALESCE(r.unread,FALSE) OR COALESCE(r.sequence,0)<COALESCE((SELECT MAX(a.sequence) FROM chat_activity a WHERE a.thread_id=t.id AND a.kind IN ('message.created','message.completed','message.delta')),0)) AS unread
FROM chat_threads t LEFT JOIN tilde_chat_reads r ON r.thread_id=t.id AND r.user_id=:user_id
WHERE EXISTS(SELECT 1 FROM chat_participants p WHERE p.thread_id=t.id AND p.agent_id=:agent AND p.active)
 AND (:user_id::UUID IS NULL OR EXISTS(SELECT 1 FROM chat_participants p WHERE p.thread_id=t.id AND p.user_id=:user_id AND p.active))
 AND NOT EXISTS(SELECT 1 FROM chat_channel_threads c WHERE c.thread_id=t.id)
 AND (:after::UUID IS NULL OR t.id>:after)
 AND (:query='' OR t.title ILIKE '%' || :query || '%' OR EXISTS(SELECT 1 FROM chat_messages m WHERE m.thread_id=t.id AND m.text ILIKE '%' || :query || '%'))
ORDER BY t.id LIMIT :limit;
--! read_state (thread, user_id, sequence, unread)
INSERT INTO tilde_chat_reads(thread_id,user_id,sequence,unread) VALUES(:thread,:user_id,:sequence,:unread)
ON CONFLICT(thread_id,user_id) DO UPDATE SET sequence=GREATEST(tilde_chat_reads.sequence,EXCLUDED.sequence),unread=EXCLUDED.unread;
--! unread (thread, user_id) : ReadState
SELECT (COALESCE(r.unread,FALSE) OR COALESCE(r.sequence,0)<COALESCE((SELECT MAX(a.sequence) FROM chat_activity a WHERE a.thread_id=:thread AND a.kind IN ('message.created','message.completed','message.delta')),0)) AS unread
FROM chat_threads t LEFT JOIN tilde_chat_reads r ON r.thread_id=t.id AND r.user_id=:user_id WHERE t.id=:thread;
--! rename (thread, title)
UPDATE chat_threads SET title=:title WHERE id=:thread;
--! agent (agent) : Agent
SELECT id,name FROM agents WHERE id=:agent AND deleted_at IS NULL;
--! latest_run (thread, agent) : Run
SELECT r.id,r.status,v.id AS invocation_id,v.status AS invocation_status FROM chat_runs r JOIN chat_invocations v ON v.run_id=r.id WHERE r.thread_id=:thread AND r.agent_id=:agent ORDER BY v.started_at DESC NULLS FIRST,v.id DESC LIMIT 1;
--! queue (thread, agent) : Queue
SELECT i.id,i.invocation_id,i.text,COALESCE(i.history_through_message_id::TEXT,'') AS history_through_message_id,i.queue_order AS position
FROM chat_inputs i JOIN chat_invocations v ON v.id=i.invocation_id WHERE v.thread_id=:thread AND v.agent_id=:agent AND NOT i.accepted ORDER BY i.queue_order,i.sequence;
--! remove_input (invocation, id)
UPDATE chat_inputs SET accepted=TRUE WHERE invocation_id=:invocation AND id=:id AND NOT accepted;
--! order_input (invocation, id, position)
UPDATE chat_inputs SET queue_order=:position WHERE invocation_id=:invocation AND id=:id AND NOT accepted;
--! sidecar_queue (thread, agent) : Queue
SELECT id,invocation_id,text,history_through_message_id,position FROM tilde_chat_sidecar_queue WHERE thread_id=:thread AND agent_id=:agent ORDER BY position;
--! clear_sidecar_queue (thread, agent)
DELETE FROM tilde_chat_sidecar_queue WHERE thread_id=:thread AND agent_id=:agent;
--! put_sidecar_queue (thread, agent, id, invocation, text, history, position)
INSERT INTO tilde_chat_sidecar_queue(thread_id,agent_id,id,invocation_id,text,history_through_message_id,position) VALUES(:thread,:agent,:id,:invocation,:text,:history,:position);
--! sequence (thread) : Sequence
SELECT activity_sequence FROM chat_threads WHERE id=:thread;

--! search_messages (thread, query, after?, limit) : MessageId
SELECT id FROM chat_messages WHERE thread_id=:thread AND text ILIKE '%' || :query || '%' AND (:after::UUID IS NULL OR id>:after) ORDER BY id LIMIT :limit;
--! cancel_message (thread, id)
UPDATE chat_messages m SET status='aborted' WHERE m.thread_id=:thread AND m.id=:id AND EXISTS(SELECT 1 FROM chat_participants p WHERE p.id=m.participant_id AND p.user_id IS NOT NULL);
