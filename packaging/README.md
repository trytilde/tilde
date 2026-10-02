# Distributing the Tilde CLI

`crates/tilde-cli` builds one binary per target in [`targets.json`](targets.json), which is the
single source of truth for the Rust triple, the GitHub release asset name, the npm platform
package and the Python wheel tags. The release workflow cross-compiles each target, then:

- `scripts/pack-cli.mjs` writes one `@trytilde/cli-<platform>` npm package per target and packs
  `@trytilde/cli` with those as `optionalDependencies`, so `npm install` fetches only the build
  that can run on the machine doing the installing.
- `scripts/pack-cli.py` writes one `trytilde-cli` wheel per wheel tag. Each carries the binary
  in the wheel's `.data/scripts` directory, so pip installs the real executable as `tilde` with
  no Python wrapper in front of it. A release has no sdist: there is nothing to build from
  source without a Rust toolchain.
- The GitHub release carries `tilde-<asset>.tar.gz` (`.zip` on Windows), which
  [`install.sh`](../install.sh) downloads.

Keep the wrapper packages thin: they locate and exec the binary and nothing else, so a bug in
them cannot change what the CLI does.
