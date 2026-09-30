# Management identities

An identity is a specific chat user. External identities retain their connection,
provider value and existing participant ID. Native Tilde identities use a unique
application subject. An optional root groups identities; it supplies no permissions.

`IdentitiesService` creates and reads identities and roots and links/unlinks them.
Channel creation resolves `agent_id` plus `provider/account-name` against that agent's
ready chat assignments, then rechecks under the connection lock. Native creation
omits both fields and uses the username type. An existing identity is reused.

`create_root` creates and links a root atomically, and cannot accompany
`root_identity_id`. Root creation can also attach existing identity IDs atomically.
Linking an identity attached elsewhere fails. Unlinking requires its expected root.
Association writes lock the chat user, with sorted locks for batch operations.

`skip_verification` records management attestation (`attested_at`) separately from
provider verification. It permits an explicit channel allow grant; it never creates
one. Provider verification remains in `AgentAccessService`.

Do preserve identity and participant IDs when grouping. Do keep SQL in `queries/`.
Do not route by root, merge histories, copy grants, or infer common ownership from
matching addresses. Root-owned connections are a later feature.
