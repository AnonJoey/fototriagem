#!/usr/bin/env python3
"""Wrapper script for fototriagem."""

import sys
from pathlib import Path

# Ensure package directory is in sys.path
BASE_DIR = Path(__file__).parent.resolve()
if str(BASE_DIR) not in sys.path:
    sys.path.insert(0, str(BASE_DIR))

from fototriagem.cli import main

if __name__ == "__main__":
    sys.exit(main())
