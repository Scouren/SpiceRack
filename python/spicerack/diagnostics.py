"""Failures and notes as a file: ``netlist`` writes a testbench's deck
and ``run`` runs it, and each leaves ``<variant>.diag.json`` (the
``spicerack.diagnostics/1`` file) in ``out_dir`` whatever happened to
the deck. Beside it: ``<variant>.source.cir``, the deck's source, which
the diagnostics' lines count in; and, for a simulator that exited
nonzero, ``<variant>.log``, everything it printed.
"""

import json
import os

from ._native import Error, _route, _version, _written

__all__ = ["SCHEMA", "netlist", "run"]

SCHEMA = "spicerack.diagnostics/1"


def netlist(testbench, dialect, out_dir, variant):
    """Writes ``testbench``'s deck for ``dialect`` and returns ``(status,
    text)``: ``("written", <the deck>)``, or ``("refused", None)`` or
    ``("failed", None)`` where ``testbench.netlist(dialect)`` raises
    ``spicerack.Error``, which this does not raise. The deck is returned,
    not written to a file; what is written to ``out_dir`` (a directory
    that exists) is the diagnostics file, ``<variant>.diag.json``, and
    the source its lines count in, ``<variant>.source.cir``.

    The file (``spicerack.diagnostics/1``): ``schema``, ``spicerack`` (the
    version), ``variant``, ``stage`` (``"netlist"``), ``dialect``,
    ``status``, ``source`` (the source file's name; ``null`` for a deck
    with no source: a ``W0040`` refusal), ``diagnostics`` (one entry per
    ``Diagnostic``, the notes of a written deck among them: ``code``,
    ``kind``, ``title``, ``cite``, ``message``, ``file`` (the source
    file's name), ``line``, ``column``, ``statement``, ``element``; the
    last five ``null`` at no statement) and ``simulator`` (``null``; for
    status ``"failed"``, as ``run`` writes it, ``message`` the failure's
    first line).

    Raises ``ValueError`` for a dialect no writer has and for a
    ``variant`` that is not a file name; ``OSError`` for a file that does
    not open. The same deck gives the same bytes.
    """
    _named(variant)
    source, text, found, error = _written(testbench, dialect, False)
    return _write(out_dir, variant, "netlist", dialect, source, found, error), text


def run(testbench, out_dir, variant):
    """Runs ``testbench`` as ``testbench.run()`` does and returns
    ``(status, text, results)``: ``("ok", <the deck that ran>, <what
    run() returns>)``, or ``"refused"`` or ``"failed"`` with ``None``
    for the results (and for a deck that was not written) where
    ``run()`` raises ``spicerack.Error``, which this does not raise. It
    writes ``netlist``'s two files for the deck of the simulator the run
    picks, and ``<variant>.log`` for a simulator that exited nonzero.

    The file is ``netlist``'s, with: ``stage``, how far the deck got:
    ``"route"`` (no simulator was picked), ``"netlist"`` (the writer of
    the picked simulator's language refused the deck) or ``"run"``;
    ``dialect``, that language (``null`` when no simulator was picked
    and the refusal is about none); ``status`` ``"ok"`` in ``"written"``'s
    place; and ``simulator``, ``null`` unless the stage is ``"route"`` or
    the status ``"failed"``: ``backend`` (the simulator's name) and
    ``dialect`` (both ``null`` when none was picked or named),
    ``exit_status`` (of a simulator that exited nonzero; else ``null``),
    ``log`` (the log file's name, or ``null``), ``message`` (of a
    simulator that exited nonzero, the first line of its output that
    mentions an error, or ``<program> exited with <status>`` when none
    does; of any other failure, the first line of its text; ``null`` for
    a refusal) and ``missing`` (``{"analyses": [...], "features":
    [...]}`` for a routing refusal, ``W0180`` to ``W0182``; else
    ``null``).

    Raises ``ValueError`` as ``run()`` does (nothing queued) and for a
    ``variant`` that is not a file name; ``OSError`` for a file that does
    not open.
    """
    _named(variant)
    stage, found = "route", []
    backend = dialect = source = text = results = error = None
    try:
        backend, dialect = _route(testbench)
        stage = "netlist"
        source, text, found, error = _written(testbench, dialect, True)
        if error is None:
            stage = "run"
            # ponytail: run() routes and writes the deck again (a Verilog block is compiled
            # again); a native run of `text` on `backend` if a batch of them shows it.
            results = testbench.run()
    except Error as e:
        error = e
    return _write(out_dir, variant, stage, dialect, source, found, error, backend), text, results


def _named(variant):
    """``variant`` names the files: one path component."""
    if not variant or os.path.basename(variant) != variant or variant in (".", ".."):
        raise ValueError(f"variant: {variant!r} is not a file name")


def _write(out_dir, variant, stage, dialect, source, found, error, backend=None):
    """Writes the files of a deck that got to ``stage`` and returns its
    status. ``found`` are the notes of the written deck; ``error`` is
    the ``spicerack.Error`` that stopped it, or ``None``."""
    out_dir = os.fspath(out_dir)
    name = None if source is None else variant + ".source.cir"
    status, simulator, log = ("ok" if stage == "run" else "written"), None, None
    if error is not None:
        status, found = error.status, list(found) + list(error.diagnostics)
        # What the error says of a simulator (`src/python/error.rs::backend`); a writer's
        # refusal says nothing of one.
        about, lang, exit_status, line, log = getattr(error, "_simulator", (None,) * 5)
        route = getattr(error, "diagnostic", None)
        dialect = dialect or lang
        if status == "failed" or stage == "route":
            simulator = {
                "backend": backend or about,
                "dialect": dialect,
                "exit_status": exit_status,
                "log": None if log is None else variant + ".log",
                "message": None if status == "refused" else line or str(error).split("\n", 1)[0],
                "missing": route["missing"] if route else None,
            }
    doc = {
        "schema": SCHEMA,
        "spicerack": _version,
        "variant": variant,
        "stage": stage,
        "dialect": dialect,
        "status": status,
        "source": name,
        "diagnostics": [
            {
                "code": d.code,
                "kind": d.kind,
                "title": d.title,
                "cite": d.cite,
                "message": d.message,
                "file": None if d.line is None else name,
                "line": d.line,
                "column": d.column,
                "statement": d.statement,
                "element": d.element,
            }
            for d in found
        ],
        "simulator": simulator,
    }
    # The diagnostics file last: where it exists, the files it names do.
    logged = simulator and simulator["log"]
    written = json.dumps(doc, indent=2) + "\n"
    for file, text in ((name, source), (logged, log), (variant + ".diag.json", written)):
        if file is not None:
            with open(os.path.join(out_dir, file), "w", encoding="utf-8", newline="\n") as f:
                f.write(text)
    return status
