--: Record(connection_id?)

--! run : Record
WITH spend AS (
 SELECT b.id,
  COALESCE((SELECT SUM(r.cost_micros) FROM inference_requests r
    LEFT JOIN chat_runs cr ON b.scope='identity' AND cr.id=r.run_id
    LEFT JOIN chat_users u ON u.id=cr.source_identity_id
    WHERE r.cost_micros IS NOT NULL
      AND ((b.scope='agent' AND r.agent_id=b.scope_id) OR (b.scope='identity' AND u.root_identity_id=b.scope_id))
      AND (b.connection_id IS NULL OR r.connection_id=b.connection_id)
      AND r.created_at >= CASE b.period WHEN 'day' THEN date_trunc('day',NOW()) WHEN 'month' THEN date_trunc('month',NOW()) ELSE '-infinity'::TIMESTAMPTZ END),0)::BIGINT AS spent,
  CASE b.period WHEN 'day' THEN date_trunc('day',NOW())+INTERVAL '1 day' WHEN 'month' THEN date_trunc('month',NOW())+INTERVAL '1 month' ELSE '9999-12-31T00:00:00Z'::TIMESTAMPTZ END AS period_end -- total budgets stay exhausted until raised or deleted
 FROM inference_budgets b
)
UPDATE inference_budgets b SET
 spent_micros=s.spent,
 exhausted_until=CASE WHEN b.action='block' AND s.spent>=b.limit_micros THEN s.period_end ELSE NULL END,
 updated_at=CASE WHEN b.spent_micros<>s.spent OR (b.exhausted_until IS NOT NULL)<>(b.action='block' AND s.spent>=b.limit_micros) THEN NOW() ELSE b.updated_at END
FROM spend s WHERE s.id=b.id
RETURNING b.id,b.scope,b.scope_id,b.connection_id,(b.exhausted_until IS NOT NULL AND b.exhausted_until>NOW()) AS exhausted;
