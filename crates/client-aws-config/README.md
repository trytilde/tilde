# client-aws-config

Copied from Tilde API's project-owned AWS credential resolver and adapted for this
workspace. Resolves credentials in order: explicit environment variables, ECS
container credentials, then the selected shared credentials file profile. Missing
credentials fail; there are no placeholder credentials or cloud/no-cloud modes.

- Environment: `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, optional `AWS_SESSION_TOKEN`.
- ECS: `AWS_CONTAINER_CREDENTIALS_FULL_URI` or `AWS_CONTAINER_CREDENTIALS_RELATIVE_URI`,
  with optional `AWS_CONTAINER_AUTHORIZATION_TOKEN`.
- Shared file: `AWS_SHARED_CREDENTIALS_FILE` or `~/.aws/credentials`, selected by
  `AWS_PROFILE` (default `default`). This reads literal key/token entries only.

The caller supplies the region. SSO, credential_process, assume-role profiles,
web identity, token-file authorization and EC2 IMDS are not implemented. Use
explicit exported credentials or an ECS role for those deployment environments.
ECS credentials are fetched again on every KMS operation, with a timeout and no
redirect following. Credential objects redact Debug and zeroize owned fields;
credential response/file buffers are zeroized on drop.

Reference: [AWS credential providers](https://docs.aws.amazon.com/sdkref/latest/guide/standardized-credentials.html).
