# ADR 0003: Deployment-owned authentication

Status: Accepted for the initial OSS engine

The standalone engine is a trusted instance with no identity or membership model.
Authentication is supplied by the deployment's ingress or private network.
Listeners default to loopback; non-loopback bindings require explicit opt-in.
Unrecognized Host and Origin headers are rejected as browser/network safeguards,
not as identity authentication. No client-supplied identity header is trusted.

A future enterprise composition can choose an engine/database and add organization
and group policies. Resource authorization must cover domain operations, background
work and agent-to-agent activity; an HTTP login check alone will not implement it.
That integration is not present in the initial OSS scope.
