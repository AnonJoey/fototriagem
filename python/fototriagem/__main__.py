"""Package execution entry point: python -m fototriagem."""

import sys
from .cli import main

if __name__ == "__main__":
    sys.exit(main())
