# ADR 0005: Reuse Tilde's focused AWS clients

Status: Accepted

Replace aws-config, aws-sdk-kms and aws-credential-types with the project-owned
client-aws-config, client-aws-sigv4 and client-aws-kms crates copied from Tilde API.
These are small infrastructure clients; the application remains a monolith with
one migration history. No S3, SES or unrelated AWS clients are brought over.

Retain the existing SigV4 signer and environment/ECS/shared-file credential
resolver. Add zeroizing owned credential and signing-key buffers and redacted
Debug output. Remove the old cloud/no-cloud split and placeholder credentials.
KMS must always perform real wrapping or fail; its former local identity/no-op
operations are not retained.

KMS retains only GenerateDataKey and Decrypt. Add the current encryption module's
required context and canonical key ID handling, typed responses, zeroizing key
buffers, explicit timeouts, disabled redirects and a fixture endpoint constructor.
The persisted wrapping key, ciphertext format and encryption context identifiers
are unchanged, so this does not require a schema or data migration.

The server requires AWS_REGION or AWS_DEFAULT_REGION for KMS. The reduced resolver
supports literal environment credentials, ECS roles and shared credentials-file
profiles. SSO, credential_process, assume-role profiles, web identity and IMDS are
not implemented. Operators using these must supply resolved temporary credentials
through the supported sources. The small HTTP client makes one attempt per call;
startup errors rely on deployment restart/retry policy.

Verification covers fixed SigV4 output, environment/ECS/file credential resolution,
real HTTP KMS wire requests, encrypted Postgres persistence and restart recovery,
including rejection of incorrect key IDs, missing fields and invalid plaintext.
Live AWS and IAM deployment configuration are outside these local fixture tests.
