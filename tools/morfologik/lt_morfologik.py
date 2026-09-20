#!/usr/bin/env python3
"""Entry point for the pure-Python LanguageTool/Morfologik dictionary tools.

Usage (from the repository root):

    python3 tools/morfologik/lt_morfologik.py <command> [options]

See ``tools/morfologik/README.md`` for the available commands.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from morfologik.cli import main  # noqa: E402

if __name__ == "__main__":
    raise SystemExit(main())
