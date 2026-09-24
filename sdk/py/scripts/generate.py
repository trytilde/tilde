#!/usr/bin/env python3
"""Generate the Python Protobuf messages and Connect stubs for the Tilde SDK.

The TypeScript workspace builds @trytilde/contracts from proto/; this is the Python equivalent.
Package builds run the same generator through the hatch hook in packages/tilde/hatch_build.py.
Output lands inside the `tilde` package (proto package `tilde.*`) and is not committed.
"""

from __future__ import annotations

import importlib.util
from pathlib import Path

root = Path(__file__).resolve().parents[3]
hook = root / "sdk/py/packages/tilde/hatch_build.py"
spec = importlib.util.spec_from_file_location("hatch_build", hook)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)  # type: ignore[union-attr]
count = module.generate(root / "proto", root / "sdk/py/packages/tilde/src")
print(f"Generated {count} contracts into sdk/py/packages/tilde/src/tilde")
