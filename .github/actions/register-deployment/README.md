# Register a Tilde deployment

Registers a release through the management API and optionally returns a masked deployment token.
It does not build or deploy your application. Gateway/Sidecar releases stay offline
until their runtime connects; Lambda registration is immediately eligible under Latest.
This is a Node 24 action with no install/build step or runtime dependencies.

## Two complete examples

- [With token output](examples/with-token-output.yml): passes the masked token to a
  deployment step in the same job. Replace the example deployment script with your
  hosting platform's command.
- [Without token output](examples/without-token-output.yml): sets `output-token: 'false'`
  and returns only the deployment ID, page link, and creation status. The user follows
  the summary link and rotates the token in Tilde for manual setup.

These files are examples, not enabled workflows. Copy the chosen file into
`.github/workflows/`. For another repository, replace the local action path with the
pinned remote reference shown in each example once the action is published.

## Workflow

Create a management API key in Tilde and store it as the GitHub Actions secret
`TILDE_API_KEY`. Configure repository/environment variables `TILDE_URL` (the management
API address) and `TILDE_AGENT_ID`. The runner must be able to reach that address; a
private Tailscale address needs a runner with access to your tailnet.

```yaml
permissions:
  contents: read

steps:
  - uses: actions/checkout@v4
  - name: Register deployment
    id: deployment
    # Local use in this repository:
    uses: ./.github/actions/register-deployment
    # From another repository, after publishing this change:
    # uses: trytilde/tilde/.github/actions/register-deployment@<full-commit-sha>
    env:
      TILDE_API_KEY: ${{ secrets.TILDE_API_KEY }}
      TILDE_URL: ${{ vars.TILDE_URL }}
      TILDE_AGENT_ID: ${{ vars.TILDE_AGENT_ID }}
    with:
      type: gateway
```

An explicit `with.api-token` overrides `TILDE_API_KEY`. An action cannot look up a
repository secret itself: the caller must pass `${{ secrets.TILDE_API_KEY }}` using
an input or `env`. `GITHUB_TOKEN` authenticates GitHub, not Tilde. Grant the key
**Edit** on the agent it deploys, from that agent's Access tab or when creating the key.
Use HTTPS when the API is reachable over a public network.

The action also accepts `url`, `ui-url`, `agent-id`, `type`, `function-arn`,
`external-id`, `label`, `repository`, `commit-sha`, `branch`, and
`rotate-existing-token`, and `output-token` (defaults to `true`); see [action.yml](action.yml). For a separate frontend URL,
set `ui-url` or `TILDE_UI_URL` so the summary links to the browser-facing service.
Lambda requires `function-arn`; deploy the immutable function version before registering.

Repository, branch/tag and commit default to GitHub context. Push event metadata adds
the commit message and author only when it matches the deployed SHA. `external-id`
defaults to `GITHUB_REPOSITORY:GITHUB_RUN_ID`, stable across reruns. Supply a distinct
external ID for multiple releases of the same agent in one workflow (e.g. matrix jobs).

## Consuming the token

Outputs are `deployment-id`, `deployment-token`, `deployment-url`, and `created`.
With `output-token: 'false'`, `deployment-token` is omitted entirely.
Pass the token to your deploy tool as an environment variable in a later step in the
**same job**, then store it as a runtime secret in your hosting platform:

```yaml
  - name: Deploy the application
    env:
      TILDE_DEPLOYMENT_TOKEN: ${{ steps.deployment.outputs.deployment-token }}
      TILDE_GATEWAY_URL: ${{ vars.TILDE_GATEWAY_URL }}
    run: ./scripts/deploy-agent.sh # Your hosting platform's deployment script
```

The action masks credentials before writing outputs, never prints responses, and never
puts the token in its summary or artifacts. Masking protects logs, not malicious later
steps: only trusted code should run in a job with management/deployment credentials.
GitHub can strip secret-bearing job outputs; do not pass this token through `jobs.*.outputs`.
Use a secret manager when different jobs need it. The output is ephemeral and is not a
place for a human to retrieve a token after the run.

## Manual setup without a secret-store integration

Set `output-token: 'false'` to prevent the action from exporting a token. Registration
still generates one server-side, but the action discards it. This mode can be retried
without rotating credentials and cannot be combined with `rotate-existing-token: 'true'`.

The job summary contains an authenticated link to the deployment table and its deployment
ID, **not a secret-bearing retrieval URL**. Sign in to Tilde, locate the deployment,
choose **Rotate token**, confirm, and copy the newly issued token directly into your
runtime configuration. Do this before starting the runtime with that token. Rotation
invalidates the CI-issued token; rotating an already used deployment requires updating
its running processes too.

Tilde stores token hashes, so the original CI token cannot be read back. This manual
flow creates a replacement. Returning the exact original token later would need a
separate encrypted, authenticated, expiring one-time retrieval feature; that feature
is not implemented here. Never expose a plaintext token in logs, summaries, or an
Actions artifact, and never base64-encode it to bypass masking.

## Retries and lost tokens

Registration is idempotent by `(agent, external-id)`. A retry returns the existing
record but no token. With token output enabled, the action fails with the deployment ID/link available and never
rotates credentials implicitly. Reuse your persisted token or explicitly set
`rotate-existing-token: 'true'` if replacing its credential is intended. If the original
registration succeeded but its response was lost, use the same recovery procedure.
Do not invent a new release ID merely to bypass an uncertain registration result.

## Verification

`task test:deployment-action` exercises real HTTP requests, CI metadata, masking,
retry/rotation behavior, redirects and error handling with local fixtures.
