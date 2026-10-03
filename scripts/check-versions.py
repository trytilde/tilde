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

for manifest in sorted((root / "sdk/py/packages").glob("*/pyproject.toml")):
    project = tomllib.loads(manifest.read_text())["project"]
    assert project["version"] == version, f"{manifest.relative_to(root)} version differs from VERSION"
    # An exact pin between Tilde's own Python packages (the SDK on the CLI) must move with the
    # release; ranges between the SDK and its adapters are deliberate and left alone.
    for dependency in project.get("dependencies", []):
        name, exact, pin = dependency.partition("==")
        if exact and name.strip().startswith("trytilde"):
            assert pin == version, (
                f"{manifest.relative_to(root)} pins {name.strip()} at {pin}, not {version}"
            )

# The platform packages the CLI publishes are generated at pack time, so the only thing to keep
# in step here is the launcher and the target table both existing.
targets = json.loads((root / "packaging/targets.json").read_text())
assert targets, "packaging/targets.json lists no targets"
for target in targets:
    for field in ("rust", "asset", "npm", "os", "cpu", "wheels"):
        assert field in target, f"packaging/targets.json entry {target} is missing {field}"
assert "@trytilde/cli" in packages, "sdk/ts/packages/cli must be a workspace package"

print(f"Release versions agree: {version}")
