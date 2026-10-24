from PySide6.QtCore import QLocale

from jase import __main__ as entry
from jase import i18n
from jase.config import Config


def test_main_resolves_language_before_window(monkeypatch, tmp_path):
    seen = {}

    class Stop(Exception):
        pass

    def fake_window(cfg, *a, **kw):
        seen["lang"] = i18n.current_language()
        raise Stop

    monkeypatch.setattr(entry, "default_config_path", lambda: tmp_path / "c.json")
    monkeypatch.setattr(entry, "load_config", lambda p: Config(language="auto"))
    monkeypatch.setattr(entry, "MainWindow", fake_window)
    monkeypatch.setattr(entry, "CountrySource", lambda: None)
    monkeypatch.setattr(entry.icons, "app_icon", lambda: None)
    monkeypatch.setattr(entry, "QApplication", lambda argv: type("A", (), {"setApplicationName": lambda self, n: None, "setWindowIcon": lambda self, i: None})())
    monkeypatch.setattr(QLocale, "system", staticmethod(lambda: QLocale("en_US")))
    try:
        entry.main()
    except Stop:
        pass
    assert seen["lang"] == "en"
    monkeypatch.setattr(entry, "load_config", lambda p: Config(language="es"))
    try:
        entry.main()
    except Stop:
        pass
    assert seen["lang"] == "es"


def test_system_locale_name_prefers_ui_languages(monkeypatch):
    class Loc:
        def __init__(self, langs, name):
            self._langs, self._name = langs, name

        def uiLanguages(self):
            return self._langs

        def name(self):
            return self._name

    # formato regional en inglés pero idioma de Windows en español
    monkeypatch.setattr(QLocale, "system", staticmethod(lambda: Loc(["es-CL", "en-US"], "en_US")))
    assert entry.system_locale_name() == "es-CL"
    assert i18n.resolve_language("auto", entry.system_locale_name()) == "es"
    monkeypatch.setattr(QLocale, "system", staticmethod(lambda: Loc([], "es_CL")))
    assert entry.system_locale_name() == "es_CL"


def _run_main_with_argv(monkeypatch, tmp_path, argv):
    sent, opened = [], []

    class App:
        def __init__(self, a):
            pass

        def setApplicationName(self, n):
            pass

        def setWindowIcon(self, i):
            pass

        def exec(self):
            return 0

    class Win:
        def __init__(self, *a, **kw):
            pass

        def show(self):
            pass

        def select_game(self, g):
            pass

        def start_link_server(self):
            return True

        def offer_find_games(self):
            pass

        def open_link(self, url):
            opened.append(url)

    monkeypatch.setattr(entry.sys, "argv", ["jase", *argv])
    monkeypatch.setattr(entry, "default_config_path", lambda: tmp_path / "c.json")
    monkeypatch.setattr(entry, "load_config", lambda p: Config(language="es"))
    monkeypatch.setattr(entry, "MainWindow", Win)
    monkeypatch.setattr(entry, "CountrySource", lambda: None)
    monkeypatch.setattr(entry.icons, "app_icon", lambda: None)
    monkeypatch.setattr(entry, "QApplication", App)
    monkeypatch.setattr(entry, "send_to_running", lambda url: sent.append(url) or True)
    monkeypatch.setattr(entry, "shutdown_lingering_workers", lambda: None)
    monkeypatch.setattr(entry.QTimer, "singleShot", staticmethod(lambda ms, fn: fn()))
    try:
        entry.main()
    except SystemExit as e:
        return sent, opened, e.code
    raise AssertionError("main debe terminar con sys.exit")


def test_invalid_or_overlong_link_is_not_sent_but_opened_locally(monkeypatch, tmp_path):
    for bad in ("jase://connect/cod2/;rm -rf:1", "jase://connect/cod2/" + "a" * 400 + ":1"):
        sent, opened, _ = _run_main_with_argv(monkeypatch, tmp_path, [bad])
        assert sent == [] and opened == [bad]


def test_valid_link_is_sent_and_exits_without_window(monkeypatch, tmp_path):
    url = "jase://connect/cod2/1.2.3.4:28960"
    sent, opened, code = _run_main_with_argv(monkeypatch, tmp_path, [url])
    assert sent == [url] and opened == [] and code == 0
