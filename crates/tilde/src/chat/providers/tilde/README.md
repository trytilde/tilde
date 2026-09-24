# Tilde chat provider

Tilde is the built-in ConnectRPC chat provider for native conversations. Its contract
is `proto/tilde/provider/tilde/v1/chat.proto`; `rpc.rs` adapts that contract to the
shared `Chat` application service. Management does not expose conversation operations.

Gateway ingress mounts `/agents/{agentId}/tilde.provider.tilde.v1.ChatService/*`.
Deployment routing validates the agent/thread-scoped ingress credential before
calling the provider, forwarding sidecar mutations to their owner. Gateway streaming
reads the durable projection; direct sidecar streaming reads the local activity log.
Both use opaque replay receipts. Runtime callbacks remain a separate invocation API.

- Do use the generated ConnectRPC client and the provider ingress listener.
- Do preserve the credential guard when composing routes; `rpc::router` is an internal
  service mount that assumes the outer ingress guard has validated resource scope.
- Do keep orchestration and transactions in the shared chat application service.
- Do not mount this service under management or add parallel management chat methods.
- Do not expose management or invocation credentials as provider credentials.


Application credentials are managed by `management.rs`; conversation methods stay in
`rpc.rs`. `credentials.rs` checks the application key against the `api_key` value of the
agent's `tilde/application` connection (see `connections/catalog/tilde`) and records the
asserted identity as an attested channel identity of that connection; `adapter.rs` is the
catalog adapter behind the generic access policy. `sessions.rs` owns session/read/queue
transactions and `db.rs` calls Cornucopia bindings under `queries/chat/tilde`.
`tilde_chat_reads` stores per-user read progress. Gateway
inputs remain in `chat_inputs`; sidecar queue snapshots are projected separately and included
in hydration so a lease transfer preserves pending work and order.

Do check membership before listing/pagination and on every resource operation. Do bind the
posting participant to the authenticated user. Do preserve the key's agent boundary for
invocation controls, even in a shared thread. Do treat `QueueChange` as a mutation under
the same lock as dispatch; a consumed item must return a conflict. Do not trust an identity
prop or an arbitrary participant ID supplied by a browser. The server-side proxy replaces
credentials and identity headers after its host authentication callback succeeds.
