#!/usr/bin/env python3
"""Release PRs also need a new blank Changie fragment for the PR check."""
from datetime import datetime
from pathlib import Path

now = datetime.now().astimezone()
path = Path('.changes/unreleased') / f'Changed-{now:%Y%m%d-%H%M%S}.yaml'
with path.open('x') as fragment:
    fragment.write(f'kind: Changed\nbody: ""\ntime: {now.isoformat()}\n')
