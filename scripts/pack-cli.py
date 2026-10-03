#!/usr/bin/env python3
"""Pack the CLI for PyPI: one `trytilde-cli` wheel per platform tag.

Usage: scripts/pack-cli.py <binaries-dir> [output-dir]

The binary is installed through the wheel's ``.data/scripts`` directory, so pip puts the real
executable on PATH as ``tilde``: there is no Python wrapper in front of it. Wheels are written
directly rather than through a build backend, because the only thing that varies between them
is the platform tag and which prebuilt binary goes in.
"""

import base64
import csv
import hashlib
import io
import json
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
NAME = "trytilde_cli"
SUMMARY = "The Tilde CLI: develop, deploy and inspect Tilde agents"

METADATA = """Metadata-Version: 2.1
Name: trytilde-cli
Version: {version}
Summary: {summary}
License: Apache-2.0
Project-URL: Homepage, https://trytilde.ai
Project-URL: Source, https://github.com/trytilde/tilde
Requires-Python: >=3.9
Description-Content-Type: text/markdown

# trytilde-cli

The [Tilde](https://trytilde.ai) CLI, as a Python package. Installing it puts the same binary
as `curl -fsSL https://trytilde.ai/install.sh | sh` on your PATH, so a project can pin the CLI
to the version of the SDK it builds against.

```bash
uv add --dev trytilde-cli
uv run tilde dev
```

`trytilde` depends on this package, so any project using the SDK already has `tilde`.
Run `tilde --help` for the commands, or read the [CLI docs](https://trytilde.ai/docs/cli).
"""

WHEEL = """Wheel-Version: 1.0
Generator: tilde pack-cli
Root-Is-Purelib: false
Tag: py3-none-{tag}
"""


def digest(data: bytes) -> str:
    encoded = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b"=")
    return f"sha256={encoded.decode()}"


def build(version: str, tag: str, binary: Path, exe: str, output: Path) -> Path:
    """Write one wheel carrying `binary` as the `tilde` script."""
    dist_info = f"{NAME}-{version}.dist-info"
    scripts = f"{NAME}-{version}.data/scripts"
    payload = binary.read_bytes()
    entries = [
        (f"{scripts}/tilde{exe}", payload),
        (f"{dist_info}/METADATA", METADATA.format(version=version, summary=SUMMARY).encode()),
        (f"{dist_info}/WHEEL", WHEEL.format(tag=tag).encode()),
    ]
    record = io.StringIO()
    writer = csv.writer(record, lineterminator="\n")
    for path, data in entries:
        writer.writerow([path, digest(data), len(data)])
    writer.writerow([f"{dist_info}/RECORD", "", ""])

    output.mkdir(parents=True, exist_ok=True)
    wheel = output / f"{NAME}-{version}-py3-none-{tag}.whl"
    with zipfile.ZipFile(wheel, "w", zipfile.ZIP_DEFLATED) as archive:
        for path, data in entries:
            info = zipfile.ZipInfo(path)
            # Scripts must stay executable once pip unpacks them.
            info.external_attr = (0o755 if path.startswith(scripts) else 0o644) << 16
            archive.writestr(info, data)
        archive.writestr(f"{dist_info}/RECORD", record.getvalue())
    return wheel


def main(argv: list[str]) -> int:
    if not argv:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    binaries = Path(argv[0]).resolve()
    output = Path(argv[1]).resolve() if len(argv) > 1 else ROOT / "dist/py"
    version = (ROOT / "VERSION").read_text().strip()
    targets = json.loads((ROOT / "packaging/targets.json").read_text())

    built = []
    for target in targets:
        exe = target.get("exe", "")
        binary = binaries / target["rust"] / f"tilde{exe}"
        if not binary.is_file():
            print(f"skipping {target['rust']}: no binary at {binary}", file=sys.stderr)
            continue
        for tag in target["wheels"]:
            built.append(build(version, tag, binary, exe, output).name)
    if not built:
        raise SystemExit(f"No CLI binaries found under {binaries}")
    print(f"Packed {len(built)} CLI wheel(s) into {output}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
