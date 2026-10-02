# @trytilde/cli

The [Tilde](https://trytilde.ai) CLI, as an npm package. It installs the same binary as
`curl -fsSL https://trytilde.ai/install.sh | sh`, so a project can pin the CLI to the version
of the SDK it builds against instead of relying on whatever is installed globally.

```bash
npm install --save-dev @trytilde/cli
npx tilde dev
```

`@trytilde/sdk` depends on this package, so any project using the SDK already has `npx tilde`.

The published package is a small launcher; the binary itself comes from one
`@trytilde/cli-<platform>` optional dependency, so you only download the build for your
machine. Set `TILDE_CLI_BINARY` to run a binary you built yourself.

Run `tilde --help` for the commands, or read the [CLI docs](https://trytilde.ai/docs/cli).
