# client-aws-kms

Adapted from Tilde API's focused KMS JSON HTTP client. This workspace retains only
GenerateDataKey (AES-256) and Decrypt, which the encryption module needs. Requests
are signed by client-aws-sigv4, using client-aws-config credentials. No AWS SDK or
Smithy dependencies, key lifecycle APIs, or no-op/plaintext wrapping modes.

Both calls include KeyId and the caller's exact encryption context. Outputs
include the canonical key ID and a zeroizing plaintext data-key buffer. Generate
also returns wrapped ciphertext. Response framing, base64 decoding and the
32-byte key length are validated; service errors never include response bodies.
Owned base64 response strings are cleared when dropped. HTTP uses TLS, disables
redirects and has a 30-second timeout. Operations are attempted once; startup
fails on an error and can be retried by the deployment's process supervisor.

`Client::new(region)` uses the regional AWS endpoint and resolves credentials on
every operation. `Client::with_config(config, endpoint)` injects credentials and
an endpoint for fixtures or compatible services; plain HTTP is restricted to
loopback. Tests exercise signed requests against a local HTTP fixture, not AWS.

References: [GenerateDataKey](https://docs.aws.amazon.com/kms/latest/APIReference/API_GenerateDataKey.html),
[Decrypt](https://docs.aws.amazon.com/kms/latest/APIReference/API_Decrypt.html).
