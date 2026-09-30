"""``python -m tilde deploy [ENTRY] ...``; see ``tilde.deploy``."""

import sys

from tilde.deploy import main

if len(sys.argv) < 2 or sys.argv[1] != "deploy":
    sys.exit("usage: python -m tilde deploy [ENTRY] [options] (see python -m tilde deploy --help)")
sys.exit(main(sys.argv[2:]))
