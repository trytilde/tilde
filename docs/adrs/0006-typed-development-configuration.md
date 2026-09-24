# ADR 0006: Development dotenv loading and typed Rust configuration

Status: Accepted

The development launcher loads the repository-root `.env` with Node's built-in
loader before spawning subprocesses. Existing process environment variables win.
A missing file is allowed when the environment already supplies configuration;
other file read failures stop development. Production startup does not load files.

`tilde::config::Config` derives envconfig::Envconfig and centralizes database,
listener, browser origins, encryption backend/material, KMS region, logging and
development port settings. Ports, addresses, booleans and backend selection are
typed. Secrets use a FromStr wrapper around zeroizing SecretString; the config
cannot be debug-printed or serialized. CLI listener/origin/backend flags override
typed environment values. AWS credential provider inputs stay in their client.

The launcher runs the Rust `check-config` subcommand before starting the API and
Vite. This exercises the real schema and encryption configuration without database
or AWS calls. Invalid settings fail before listeners start and never echo secret
values. API_PORT and WEB_PORT must be valid, distinct nonzero development ports.
Explicitly choosing WEB_PORT handles an occupied default port; Vite retains strict
port selection so its URL cannot diverge from the API's browser-origin settings.
