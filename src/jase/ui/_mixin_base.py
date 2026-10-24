"""Base de tipado de los mixins de MainWindow.

En ejecución es ``object`` (los mixins no son QObject). Solo para mypy es QMainWindow, así
``self`` vale como padre de diálogos y menús y se resuelven ``statusBar()``, ``sender()``, etc.
"""
from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from PySide6.QtWidgets import QMainWindow as MixinBase
else:
    MixinBase = object
