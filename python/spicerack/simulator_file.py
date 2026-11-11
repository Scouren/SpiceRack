"""Simulator descriptors as files: ``load_simulator(path)`` reads a
``spicerack.simulator/1`` TOML file into a ``Simulator``. The file's keys
are ``Simulator``'s arguments, so a file and a call are checked by the one
constructor; only ``schema`` is the file's own.
"""

from ._native import Simulator

__all__ = ["SCHEMA", "load_simulator"]

SCHEMA = "spicerack.simulator/1"


def load_simulator(path):
    """The ``Simulator`` the TOML file at ``path`` describes: read and
    checked, not registered (``register_simulator`` does that).

    Keys: ``schema = "spicerack.simulator/1"`` (required), then
    ``Simulator``'s arguments by name: ``name``, ``dialect``, ``command``,
    ``output``, ``analyses``, ``features``, ``verilog``, ``verilog_a``.

    Raises ``ValueError`` (code ``R0040``, naming the key) for a missing or
    different ``schema``, a key that is none of those, or a value
    ``Simulator`` refuses; ``OSError`` for a file that does not open;
    ``tomllib.TOMLDecodeError`` for text that is not TOML. Needs Python
    3.11 (``tomllib``).
    """
    import tomllib  # here, not at the top: `import spicerack` still works on Python 3.10

    with open(path, "rb") as f:
        table = tomllib.load(f)
    schema = table.pop("schema", None)
    if schema != SCHEMA:
        # The catalogue's title for R0040 (`src/syntax/diag/rows_r.rs`), as `Simulator` words it.
        raise ValueError(
            "R0040 a simulator descriptor that does not validate: "
            f"schema: {schema!r} is not {SCHEMA!r}"
        )
    return Simulator(**table)
