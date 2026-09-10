#!/usr/bin/env bash
set -euo pipefail

base_ref="${1:-${GITHUB_BASE_REF:-main}}"

if [[ "$base_ref" != *"..."* ]]; then
	if git rev-parse --verify --quiet "origin/${base_ref}" >/dev/null; then
		base_ref="origin/${base_ref}...HEAD"
	else
		base_ref="${base_ref}...HEAD"
	fi
fi

python3 - "$base_ref" <<'PY'
import re
import subprocess
import sys
from pathlib import Path

diff_ref = sys.argv[1]
def git_stdout(*args):
    return subprocess.run(
        ["git", *args],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout


try:
    # The initial repository has no HEAD yet; inspect its index and working tree.
    has_head = subprocess.run(
        ["git", "rev-parse", "--verify", "HEAD"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    ).returncode == 0
    diffs = [git_stdout("diff", "--name-status", diff_ref)] if has_head else []
    diffs += [git_stdout("diff", "--name-status", "--cached"), git_stdout("diff", "--name-status")]
    untracked = git_stdout("ls-files", "--others", "--exclude-standard")
except subprocess.CalledProcessError as exc:
    print(f"failed to inspect git state for {diff_ref}: {exc}", file=sys.stderr)
    sys.exit(exc.returncode)

added = []
seen = set()
for diff in diffs:
    for line in diff.splitlines():
        parts = line.split("\t")
        if len(parts) < 2 or parts[0] != "A":
            continue
        path = parts[1]
        if re.fullmatch(r"\.changes/unreleased/[^/]+\.ya?ml", path) and path not in seen:
            seen.add(path)
            added.append(Path(path))

for path in untracked.splitlines():
    if re.fullmatch(r"\.changes/unreleased/[^/]+\.ya?ml", path) and path not in seen:
        seen.add(path)
        added.append(Path(path))

if not added:
    print(
        "missing Changie changeset: add a new .changes/unreleased/*.yaml file. "
        'For no release note, add a blank acknowledgement with body: "".',
        file=sys.stderr,
    )
    sys.exit(1)

valid_kinds = set()
for line in Path(".changie.yaml").read_text(encoding="utf-8").splitlines():
    match = re.match(r"\s*-\s+label:\s+(.+?)\s*$", line)
    if match:
        valid_kinds.add(match.group(1).strip().strip("'\""))

if not valid_kinds:
    print("could not read Changie kinds from .changie.yaml", file=sys.stderr)
    sys.exit(1)

failed = False
for path in added:
    text = path.read_text(encoding="utf-8")
    kind_match = re.search(r"(?m)^kind:\s*(.+?)\s*$", text)
    if not kind_match:
        print(f"{path}: missing kind field", file=sys.stderr)
        failed = True
        continue
    kind = kind_match.group(1).strip().strip("'\"")
    if kind not in valid_kinds:
        print(
            f"{path}: kind {kind!r} is not one of {sorted(valid_kinds)}",
            file=sys.stderr,
        )
        failed = True

    if not re.search(r"(?m)^body:\s*", text):
        print(f"{path}: missing body field; use body: \"\" for blank acknowledgements", file=sys.stderr)
        failed = True

if failed:
    sys.exit(1)

for path in added:
    print(f"found Changie changeset: {path}")
PY
