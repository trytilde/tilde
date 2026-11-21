-- The hosts a provider's API answers at: the only hosts its connections' request credentials
-- (the ontology control plane's credentials endpoint) may be sent to.
ALTER TABLE connection_providers ADD COLUMN api_hosts TEXT[] NOT NULL DEFAULT '{}';
