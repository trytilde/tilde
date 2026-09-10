#!/usr/bin/env python3
"""Check Changie's shared version at package-manager boundaries (Python 3.11+)."""

import json
from pathlib import Path
import tomllib

root = Path(__file__).resolve().parent.parent
version = (root / "VERSION").read_text().strip()
workspace = tomllib.loads((root / "Cargo.toml").read_text())
assert workspace["workspace"]["package"]["version"] == version, "Cargo.toml version differs from VERSION"
assert (root / f".changes/v{version}.md").is_file(), "Missing Changie version file"

lock = tomllib.loads((root / "Cargo.lock").read_text())
for member in workspace["workspace"]["members"]:
    package = tomllib.loads((root / member / "Cargo.toml").read_text())["package"]
    assert package["version"] == {"workspace": True}, f"{member} must inherit workspace version"
    locked = [p for p in lock["package"] if p["name"] == package["name"] and "source" not in p]
    assert len(locked) == 1 and locked[0]["version"] == version, f"Cargo.lock stale for {member}"

packages = {}
for manifest in sorted((root / "sdk/ts/packages").glob("*/package.json")):
    package = json.loads(manifest.read_text())
    assert package.get("version") == version, f"{manifest.relative_to(root)} version differs from VERSION"
    packages[package["name"]] = package

for name, package in packages.items():
    for field in ("dependencies", "devDependencies", "peerDependencies", "optionalDependencies"):
        for dependency, spec in package.get(field, {}).items():
            if dependency in packages:
                assert spec in ("workspace:*", "workspace:^", "workspace:~"), (
                    f"{name}: use an unversioned workspace protocol for {dependency}; "
                    "pnpm resolves it to the release version when packing/publishing"
                )

print(f"Release versions agree: {version}")
