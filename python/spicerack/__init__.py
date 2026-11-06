"""Python package wrapper for the SpiceRack native extension."""

from ._native import *
from . import dialects
from .def_ import Answer, Card
from .dialects import Dialect, register_dialect
