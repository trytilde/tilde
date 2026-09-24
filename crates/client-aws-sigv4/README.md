# client-aws-sigv4

Small, auditable AWS Signature Version 4 implementation used by Tilde's focused AWS HTTP clients. It replaces `aws-sigv4`, `aws-credential-types`, and the Smithy runtime for request shapes used in this workspace.

The implementation follows the [AWS Signature Version 4 documentation](https://docs.aws.amazon.com/IAM/latest/UserGuide/create-signed-request.html).

## Public surface

- `Credentials`: access key, secret key, and optional session token.
- `Signer`: signs a `reqwest::Request` or creates a query-presigned request.
- `PresignedRequest`: URL plus headers covered by its signature.

## Updating

Compare changes with the AWS documentation and current `aws-sigv4` canonical-request behavior. Add a fixed-time test vector for every changed rule. Keep this focused: add only operations exercised by workspace call sites.

Copied from Tilde API into this workspace. Credential fields and derived signing
key buffers now zeroize on drop, and credential Debug output is redacted. Fixed
time signing tests include an independently computed complete signature vector.
