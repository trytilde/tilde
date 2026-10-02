# trytilde-cli

The [Tilde](https://trytilde.ai) CLI, as a Python package. Installing it puts the same binary as
`curl -fsSL https://trytilde.ai/install.sh | sh` on your PATH as `tilde`.

```bash
uv add --dev trytilde-cli
uv run tilde dev
```

`trytilde` depends on this package, so any project using the SDK already has `tilde`.

Inside this repository the package runs the binary you built with
`cargo build -p tilde-cli`; released wheels carry a prebuilt binary per platform.
