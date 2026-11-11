"""Results as files: ``export(results, stem)`` writes ``<stem>.raw`` (one
ngspice raw file, the plots of every analysis) and ``<stem>.meas.json``
(the ``spicerack.measures/1`` sidecar: what a raw file has no place for).
"""

import json
import math
import os

from ._native import _export

__all__ = ["SCHEMA", "export"]

SCHEMA = "spicerack.measures/1"


def export(results, stem, *, raw_format="binary"):
    """Writes ``results`` as ``<stem>.raw`` and ``<stem>.meas.json`` and
    returns the two paths. ``results`` is a sequence of results: what
    ``Testbench.run()`` returns (per analysis a result, a list of one
    result per step, or ``None``, which is left out), single-analysis
    results in a list, or ``read_raw()``'s plots.

    The raw file is ngspice's format (``raw_format``: ``"binary"`` or
    ``"ascii"``), the plots in the order given, a stepped analysis's one
    after the other. Every vector has its canonical name, the one a result
    is indexed by, whichever simulator ran: ``out``, ``i(vin)``, ``time``.

    The sidecar (``spicerack.measures/1``): ``schema``, ``spicerack`` (the
    version), ``backend`` (``null`` unless one simulator family wrote
    every plot), ``raw`` (the raw file's name), ``analyses`` (``index``:
    the number of its first plot in the raw file; ``kind``: ``tran``,
    ``ac``, ..., ``null`` for a plot read from a file; ``plot``: its plot's
    name; ``steps``: ``null``, or one ``{parameter: value}`` per step) and
    ``measures`` (``name``; ``analysis``: its place in ``analyses``;
    ``step``: its place in that analysis's ``steps``, or ``null``;
    ``value``: ``null`` exactly when ``status`` is not ``"ok"``;
    ``status``: ``"ok"`` or ``"failed"``; ``reason``, only beside a value
    JSON cannot hold), ordered by analysis, step, then the deck's order.

    Raises ``ValueError`` for a ``raw_format`` that is neither, for
    results with no plot among them and for a result no raw file holds (no
    vectors, a vector name that is not one word); ``TypeError`` for an
    item that is no result; ``OSError`` for a file that does not open.
    The same results give the same bytes.
    """
    if raw_format not in ("binary", "ascii"):
        raise ValueError(f"raw_format: {raw_format!r} is not 'binary' or 'ascii'")
    stem = os.fspath(stem)
    raw, version, analyses = _export(results, raw_format == "ascii")
    if not analyses:
        raise ValueError("export: no result holds a plot")
    kinds = {a[0] for a in analyses}
    backends = {a[2] for a in analyses}
    doc = {
        "schema": SCHEMA,
        "spicerack": version,
        "backend": backends.pop() if len(backends) == 1 else None,
        "raw": os.path.basename(stem) + ".raw",
        "analyses": [],
        "measures": [],
    }
    strays = set()
    for place, (kind, plot, _, first, points, steps) in enumerate(analyses):
        doc["analyses"].append({
            "index": first,
            "kind": kind,
            "plot": plot,
            "steps": None if points is None else [dict(p) for p in points],
        })
        for k, rows in enumerate(steps):
            step = None if points is None else k
            for name, word, value in rows:
                if word != kind:
                    # Another analysis's measure (every plot of a run carries the run's): that
                    # analysis writes it, or, when it is not exported, the first plot holding it.
                    if word in kinds or (name, word, step) in strays:
                        continue
                    strays.add((name, word, step))
                ok = value is not None and math.isfinite(value)
                row = {
                    "name": name,
                    "analysis": place,
                    "step": step,
                    "value": value if ok else None,
                    "status": "ok" if ok else "failed",
                }
                if value is not None and not ok:
                    row["reason"] = f"the value {value} is not finite"
                doc["measures"].append(row)
    # Both files' bytes exist before either is written.
    text = json.dumps(doc, indent=2, allow_nan=False) + "\n"
    raw_path, meas_path = stem + ".raw", stem + ".meas.json"
    with open(raw_path, "wb") as f:
        f.write(raw)
    with open(meas_path, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    return raw_path, meas_path
