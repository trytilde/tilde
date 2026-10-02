"""Run the `tilde` binary built from `crates/tilde-cli` in this checkout.

Only used inside this repository: the published wheels install the binary itself as the `tilde`
script, with no Python in front of it. See this package's pyproject.toml.
"""

import os
import sys
from pathlib import Path


def binary() -> Path:
    if override := os.environ.get("TILDE_CLI_BINARY"):
        return Path(override)
    # sdk/py/packages/tilde-cli/src/trytilde_cli/__init__.py -> repository root
    root = Path(__file__).resolve().parents[5]
    for profile in ("release", "debug"):
        candidate = root / "target" / profile / "tilde-cli"
        if candidate.is_file():
            return candidate
    raise SystemExit(
        "tilde: no CLI binary in this checkout. Build it with `cargo build -p tilde-cli`, "
        "or set TILDE_CLI_BINARY."
    )


def main() -> int:
    path = binary()
    os.execv(str(path), [str(path), *sys.argv[1:]])
