"""``python -m tilde declarations [ENTRY]``; see ``tilde.declarations``."""

import sys

from tilde.declarations import main

if len(sys.argv) < 2 or sys.argv[1] != "declarations":
    sys.exit(
        "usage: python -m tilde declarations [ENTRY]\n"
        "Run `tilde deploy` to register a deployment (see https://trytilde.ai/docs/cli)."
    )
sys.exit(main(sys.argv[2:]))
