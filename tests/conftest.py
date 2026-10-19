import os

import pytest

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")


@pytest.fixture(autouse=True)
def _spanish_ui():
    """Los tests comparan texto en español: se fuerza antes de cada uno."""
    from jase import i18n
    i18n.set_language("es")
