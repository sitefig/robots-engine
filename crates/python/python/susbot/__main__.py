"""The ``susbot`` command: the Rust CLI, run in-process.

``python -m susbot https://example.com`` and the ``susbot`` script installed
with the package are the same program as ``cargo install susbot``.
"""

from __future__ import annotations

import signal
import sys
from typing import Sequence

from . import _susbot


def main(argv: Sequence[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    # The CLI runs in Rust with the GIL released; let Ctrl-C stop it at once
    # instead of waiting for Python to regain control.
    try:
        signal.signal(signal.SIGINT, signal.SIG_DFL)
    except ValueError:  # not the main thread
        pass
    sys.stdout.flush()
    sys.stderr.flush()
    return _susbot.run_cli(["susbot", *args])


if __name__ == "__main__":
    sys.exit(main())
