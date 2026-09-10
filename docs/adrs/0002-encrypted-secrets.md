# ADR 0002: Internal encryption with seed or KMS key protection

Status: Accepted

Stored secrets use AES-256-GCM authenticated encryption with fresh nonces. The
context binds version, data-key identifier, resource kind, resource ID and field
name. Ciphertext is never treated as authorization. No public decryption RPC exists.

A single data key is initialized once under a Postgres advisory lock and retained
as wrapped bytes in the database. Seed protection uses externally supplied random
32-byte material. KMS protection uses the project-owned client-aws-kms client with an existing
symmetric KMS key and exact encryption context. The same ciphertext storage format
works with either backend. Failure to unlock is fatal; no no-op/plaintext fallback
or automatic replacement key is permitted.

The unwrapped data key is cached in a zeroizing buffer. Application plaintext uses
SecretString/Zeroizing, and AES/GHASH/POLYVAL zeroization features are enabled.
Transport/environment copies are outside that guarantee. Runtime KMS revocation
does not invalidate already-cached plaintext keys.

Key rotation and backend migration require an explicit future rewrapping process.
Their absence must stay documented so operators retain the original recovery keys.
There are no organization/team fields, key-management APIs, alias registries or
separate migration runners in this module.
