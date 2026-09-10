# Embedded Rotel fork

Upstream: https://github.com/rotel-dev/rotel
Revision: e52776089f6649127ff747e8fb8740369d7e93ca
License: Apache-2.0 (retained in LICENSE).

This is a source-vendored library fork, not a separately published GitHub fork.
The upstream source, protobuf definitions and processor SDKs are retained. CLI
binaries, benches and dev dependencies are not built by this package. Tilde uses
`default-features = false`; it does not load Rust/Python processor plugins.

Local integration changes:

- Expose the HTTP Tower service builder and its concrete types so Tilde can
  compose authentication, admission control and timeouts on its existing listener.
- Accept Send bodies without an unnecessary Sync bound, for Axum compatibility.
- Add optional decoded-request validation and downstream acknowledgement to
  OTLPOutput. Validation happens before splitting. Tilde acknowledges only after
  Postgres commit; rejected requests use non-retryable HTTP responses.
- Bound decoded HTTP bodies to 4 MiB and reject malformed content-type headers
  without panicking.
- Split large messages before offering them to the batch buffer, avoiding the
  upstream limit of two batches per input request.
- Move exact batch fits without splitting so no empty acknowledgement reference
  remains waiting forever in a zero-span batch.
- Drain pending sends, partial batches and queued messages on shutdown.

Application-level Postgres persistence, token authorization and durable
external delivery are implemented in crates/tilde/src/tracing. Keep those policies
out of the fork. Upgrade by comparing these files against the pinned upstream
revision and running the tracing integration tests before changing the revision.
