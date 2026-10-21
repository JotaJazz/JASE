import os

import keyring
import pytest
from keyring.backend import KeyringBackend
from keyring.errors import PasswordDeleteError

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")


@pytest.fixture(autouse=True)
def _spanish_ui():
    """Los tests comparan texto en español: se fuerza antes de cada uno."""
    from jase import i18n
    i18n.set_language("es")


class MemKeyring(KeyringBackend):
    """Keyring en memoria: ningún test toca el llavero real del sistema."""
    priority = 1

    def __init__(self):
        self.d = {}

    def get_password(self, s, u):
        return self.d.get((s, u))

    def set_password(self, s, u, p):
        self.d[(s, u)] = p

    def delete_password(self, s, u):
        if (s, u) not in self.d:
            raise PasswordDeleteError()
        del self.d[(s, u)]


@pytest.fixture(autouse=True)
def mem_keyring():
    previous = keyring.get_keyring()
    kr = MemKeyring()
    keyring.set_keyring(kr)
    yield kr
    keyring.set_keyring(previous)
