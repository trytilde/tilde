-- Prices are reconciled from the vendored LiteLLM sheet at startup (source 'litellm'); rows with
-- source 'manual' are operator overrides and are never touched by reconciliation.
CREATE TABLE inference_prices (
 provider_id TEXT NOT NULL,
 model TEXT NOT NULL,
 input_per_m_micros BIGINT,
 output_per_m_micros BIGINT,
 cached_input_per_m_micros BIGINT,
 cache_write_per_m_micros BIGINT,
 -- Non-token units: micro-dollars per generated image, and per million input characters (speech).
 image_micros BIGINT,
 character_per_m_micros BIGINT,
 second_micros BIGINT,
 source TEXT NOT NULL CHECK (source IN ('litellm','manual')),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 PRIMARY KEY (provider_id, model)
);
-- Cost is priced at insert time from the table; NULL means no price was known for the model.
-- `units` is images generated, characters synthesised or audio seconds transcribed for kinds billed without tokens.
ALTER TABLE inference_requests ADD COLUMN cache_write_tokens BIGINT, ADD COLUMN units BIGINT, ADD COLUMN cost_micros BIGINT;
-- Settlement sums per agent, per connection and per run's identity; these keep those sums indexed.
CREATE INDEX inference_requests_connection ON inference_requests(connection_id, created_at DESC);
CREATE INDEX inference_requests_run ON inference_requests(run_id);

-- A spend ceiling for one agent or one chat identity over a rolling period. The budget worker
-- sums request costs and sets exhausted_until; token issue and renewal then withhold every
-- inference connection for a blocked scope, so enforcement never touches the request path.
CREATE TABLE inference_budgets (
 id UUID PRIMARY KEY,
 scope TEXT NOT NULL CHECK (scope IN ('agent','identity')),
 scope_id UUID NOT NULL,
 -- Set on an agent budget to cap spend on one connection only; NULL caps the whole scope.
 connection_id UUID,
 period TEXT NOT NULL CHECK (period IN ('day','month','total')),
 limit_micros BIGINT NOT NULL CHECK (limit_micros >= 0),
 action TEXT NOT NULL CHECK (action IN ('block','flag')),
 spent_micros BIGINT NOT NULL DEFAULT 0,
 exhausted_until TIMESTAMPTZ,
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 CHECK (connection_id IS NULL OR scope='agent')
);
CREATE UNIQUE INDEX inference_budget_scope ON inference_budgets(scope, scope_id, period, COALESCE(connection_id, '00000000-0000-0000-0000-000000000000'));
-- Spend totals change constantly; only a change in exhaustion is worth waking configuration readers.
CREATE TRIGGER sidecar_inference_budget_notify AFTER INSERT OR DELETE ON inference_budgets FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_configuration');
CREATE TRIGGER sidecar_inference_budget_exhaustion_notify AFTER UPDATE ON inference_budgets FOR EACH ROW WHEN (OLD.exhausted_until IS DISTINCT FROM NEW.exhausted_until OR OLD.action IS DISTINCT FROM NEW.action) EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_configuration');
