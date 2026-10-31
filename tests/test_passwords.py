import keyring
from keyring.backend import KeyringBackend
from conftest import MemKeyring
from jase import passwords
from jase.models import ServerAddress

def test_suite_uses_memory_keyring():
    assert isinstance(keyring.get_keyring(), MemKeyring)

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


def test_steam_key_roundtrip_and_clear():
    kr = MemKeyring(); keyring.set_keyring(kr)
    assert passwords.get_steam_key() == ""
    passwords.set_steam_key("ABC")
    assert kr.d[("JimmysAllSeeingEye", "steam:webapi")] == "ABC"
    assert passwords.get_steam_key() == "ABC"
    passwords.set_steam_key("")
    assert passwords.get_steam_key() == ""
    passwords.set_steam_key("")  # borrar de nuevo no falla


def test_steam_key_get_tolerates_keyring_failure(monkeypatch):
    from keyring.errors import KeyringError

    def boom(*a):
        raise KeyringError("x")
    monkeypatch.setattr(keyring, "get_password", boom)
    assert passwords.get_steam_key() == ""


def test_set_password_empty_deletes():
    kr = MemKeyring(); keyring.set_keyring(kr)
    a = ServerAddress("1.2.3.4", 28960)
    passwords.set_password("cod2", a, "pw")
    passwords.set_password("cod2", a, "")
    assert passwords.get_password("cod2", a) is None
