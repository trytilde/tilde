# ADR 0004: Caller-supplied webhook signing key on the agent

Status: Accepted

An agent has a name, an optional endpoint, and a required webhook signing key
supplied by the caller during creation. It has no description or collection of
named secrets. The public service exposes create, get, list, update and delete;
there are no secret-management or key-read methods. Updates change the name or
endpoint and preserve the signing key.

The key is an opaque shared secret, matching Tilde's use of the literal key bytes
for HMAC-SHA256. The old `tilde_whsec_` prefix is accepted but not required. Callers
securely generate 32-1024 printable non-whitespace ASCII characters and configure
the same value in the target agent. This registry does not generate a default key
or implement outbound webhook delivery yet.

The encryption module seals the key before insertion into one NOT NULL BYTEA
column, `agents.webhook_signing_key`. The binary envelope contains the big-endian
format version (2 bytes), data-key UUID (16), nonce (12), and ciphertext including
the authentication tag. Its authenticated context binds the value to the agent ID
and the `webhook_signing_key` field. No plaintext or ciphertext appears in public
agent responses. Application-owned plaintext buffers use SecretString/Zeroizing.

Creation with a caller-supplied agent ID is retryable only when the name, endpoint
and signing key match. The retry path decrypts the existing column, compares the
key with a constant-time comparison and drops the plaintext. Different keys
produce AlreadyExists rather than replacing the stored value.

This workspace is still unreleased and has no deployed schema to migrate, so the
initial central migration is revised directly. Removed Protobuf field names and
numbers are reserved. SQLx offline metadata and generated clients are regenerated
from the new schema and contract.
