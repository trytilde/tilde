-- The registry's OpenID Connect issuer signs tokens for relying parties (first: an `aws`/`oidc`
-- connection's web identity for STS). Each RSA signing key's PKCS#8 private key is sealed by the
-- encryption crate (resource `oidc_signing_key`, the key's ID, which is also its JWKS `kid`).
-- The newest key signs; keys stay published until they age out of the JWKS window.
CREATE TABLE oidc_signing_keys (
 id UUID PRIMARY KEY,
 sealed BYTEA NOT NULL,
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX oidc_signing_keys_recent ON oidc_signing_keys(created_at DESC);
