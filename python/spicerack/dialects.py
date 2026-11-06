"""Dialect plugins: a dialect is a ``Dialect`` subclass whose ``define(d)``
makes the definition API's calls (``spicerack.def_.DialectDef``, the Rust
calls 1:1) and ``register_dialect`` registers it.

``NGSPICE`` ... ``VERILOG`` are the built-ins as classes: each ``define`` is
``d.extends(Lang.X)``, so ``class MyHspice(HSPICE)`` with
``super().define(d)`` extends HSPICE.
"""

from .def_ import DialectDef, Lang, register

__all__ = [
    "Dialect", "register_dialect",
    "NGSPICE", "ESPICE", "HSPICE", "XYCE", "LTSPICE", "SPECTRE", "VACASK", "CDL", "VERILOG",
]


class Dialect:
    """A dialect named ``name``. Subclasses override ``define``.

    A class attribute named as a ``DialectDef`` call is that call with the
    attribute as its one argument (``extends = Lang.NGSPICE``,
    ``fold_case = True``), applied before ``define`` in declaration order,
    base classes first.
    """

    def __init__(self, name):
        self.name = name

    def define(self, d):
        """The definition's calls on ``d``; returns the definition."""
        return d


def register_dialect(obj):
    """Registers ``obj`` (a ``Dialect`` instance): ``DialectDef(obj.name)``,
    its one-argument class attributes, ``obj.define(d)``, then ``register``.
    Returns its ``Lang``; registering an equal definition again returns the
    same one, another under the same name raises ``ValueError``."""
    d = DialectDef(obj.name)
    for cls in reversed(type(obj).__mro__):
        for k, v in vars(cls).items():
            if not k.startswith("_") and not callable(v) and hasattr(DialectDef, k):
                d = getattr(d, k)(v)
    d = obj.define(d)
    if not isinstance(d, DialectDef):
        raise TypeError(f"{type(obj).__name__}.define must return the DialectDef")
    return register(d)


def _builtin(lang, doc):
    class Builtin(Dialect):
        def define(self, d):
            return d.extends(lang)

    Builtin.__doc__ = doc
    return Builtin


NGSPICE = _builtin(Lang.NGSPICE, "ngspice, as a base class.")
ESPICE = _builtin(Lang.ESPICE, "ESPice, as a base class.")
HSPICE = _builtin(Lang.HSPICE, "HSPICE, as a base class.")
XYCE = _builtin(Lang.XYCE, "Xyce, as a base class.")
LTSPICE = _builtin(Lang.LTSPICE, "LTspice, as a base class.")
SPECTRE = _builtin(Lang.SPECTRE, "Spectre, as a base class.")
VACASK = _builtin(Lang.VACASK, "VACASK, as a base class.")
CDL = _builtin(Lang.CDL, "CDL, as a base class.")
VERILOG = _builtin(Lang.VERILOG, "Verilog, as a base class.")
for _n in __all__[2:]:
    globals()[_n].__name__ = globals()[_n].__qualname__ = _n
