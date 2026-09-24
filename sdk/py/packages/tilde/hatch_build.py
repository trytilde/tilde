"""Generate the Connect and protobuf modules during builds, so wheels never miss contracts.

`proto/` at the repository root is the source. Builds from an sdist have no proto/ but do
carry the generated modules as artifacts, so the hook is a no-op there.
"""

from __future__ import annotations

import shutil
import subprocess
import sys
from pathlib import Path

from hatchling.builders.hooks.plugin.interface import BuildHookInterface

PACKAGE = Path(__file__).resolve().parent
GENERATED = (
    "agent_event_ingress",
    "agent_host",
    "ingress",
    "management",
    "provider",
    "run",
    "runtime",
    "setup",
    "types",
)


def generate(proto: Path, src: Path) -> int:
    """Run protoc with the Python and Connect plugins; returns the number of proto files."""
    plugin = Path(sys.executable).parent / "protoc-gen-connectrpc"
    if not plugin.is_file():
        raise RuntimeError("protoc-gen-connectrpc is missing from the build environment")
    files = sorted(proto.rglob("*.proto"))
    package = src / "tilde"
    for name in GENERATED:
        shutil.rmtree(package / name, ignore_errors=True)
    subprocess.run(
        [
            sys.executable,
            "-m",
            "grpc_tools.protoc",
            f"-I{proto}",
            f"--python_out={src}",
            f"--pyi_out={src}",
            f"--plugin=protoc-gen-connectrpc={plugin}",
            f"--connectrpc_out={src}",
            "--connectrpc_opt=protobuf=google",
            *map(str, files),
        ],
        check=True,
    )
    for directory in package.rglob("*"):
        if directory.is_dir() and directory.relative_to(package).parts[0] in GENERATED:
            (directory / "__init__.py").touch()
    return len(files)


class GenerateContracts(BuildHookInterface):
    PLUGIN_NAME = "custom"

    def initialize(self, version: str, build_data: dict) -> None:
        proto = PACKAGE.parents[3] / "proto"
        src = PACKAGE / "src"
        if proto.is_dir():
            generate(proto, src)
        elif not (src / "tilde" / "types").is_dir():
            raise RuntimeError("Generated contracts are missing and proto/ is unavailable")
        build_data.setdefault("artifacts", []).extend(f"src/tilde/{name}/**" for name in GENERATED)
