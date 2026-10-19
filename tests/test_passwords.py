import keyring
import pytest
from keyring.backend import KeyringBackend
from jase import passwords
from jase.models import ServerAddress

class MemKeyring(KeyringBackend):
    priority = 1
    def __init__(self): self.d = {}
    def get_password(self, s, u): return self.d.get((s, u))
    def set_password(self, s, u, p): self.d[(s, u)] = p
    def delete_password(self, s, u):
        from keyring.errors import PasswordDeleteError
        if (s, u) not in self.d: raise PasswordDeleteError()
        del self.d[(s, u)]

@pytest.fixture(autouse=True)
def restore_keyring():
    previous = keyring.get_keyring()
    yield
    keyring.set_keyring(previous)

def test_set_get_forget():
    kr = MemKeyring(); keyring.set_keyring(kr)
    a = ServerAddress("1.2.3.4", 28960)
    assert passwords.get_password("cod2", a) is None
    passwords.set_password("cod2", a, "pw")
    assert kr.d[("JimmysAllSeeingEye", "cod2:1.2.3.4:28960")] == "pw"
    assert passwords.get_password("cod2", a) == "pw"
    passwords.forget_password("cod2", a); passwords.forget_password("cod2", a)  # 2ª vez no falla
    assert passwords.get_password("cod2", a) is None

class BrokenKeyring(KeyringBackend):
    priority = 1
    def get_password(self, s, u):
        from keyring.errors import KeyringError
        raise KeyringError("boom")
    def set_password(self, s, u, p):
        from keyring.errors import KeyringError
        raise KeyringError("boom")
    def delete_password(self, s, u):
        from keyring.errors import KeyringError
        raise KeyringError("boom")

def test_backend_errors_are_tolerated():
    keyring.set_keyring(BrokenKeyring())
    a = ServerAddress("1.2.3.4", 28960)
    assert passwords.get_password("cod2", a) is None
    passwords.forget_password("cod2", a)
