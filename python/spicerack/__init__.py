"""Python package wrapper for the SpiceRack native extension."""

from ._native import *
from . import dialects
from .def_ import Answer, Card
from .dialects import Dialect, register_dialect
from .export import export
from .simulator_file import load_simulator

#: The names outside the 1.0 freeze: each may change in a minor release.
provisional = ("Simulator.read",)
