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
    monkeypatch.setattr(entry, "QApplication", lambda argv: type("A", (), {"setApplicationName": lambda self, n: None})())
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
