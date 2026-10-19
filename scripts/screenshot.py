"""Genera las capturas de pantalla del README con datos de ejemplo inventados.

Uso: .venv/bin/python scripts/screenshot.py [--lang en|es] [--out PATH]
Corre con Qt offscreen, config temporal y sin red: los servidores, los jugadores
y los países son ficticios (IPs solo de los rangos de documentación RFC 5737).
"""
from __future__ import annotations

import argparse
import os
import sys
import tempfile
import time
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtCore import Qt
from PySide6.QtWidgets import QApplication

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "src"))

from jase import i18n  # noqa: E402
from jase.config import Config  # noqa: E402
from jase.models import Player, ServerAddress, ServerInfo  # noqa: E402
from jase.scanner import ScanResult  # noqa: E402
from jase.ui.main_window import MainWindow  # noqa: E402
from jase.ui.server_model import COL_COUNTRY  # noqa: E402

GAME = "cod2"
SIZE = (1280, 800)
SELECTED_HOST = "192.0.2.14"

# host, nombre, mapa, gametype, jugadores, máx, ping, mod, país
_SERVERS = [
    ("192.0.2.14", "^2[NOX] ^7Clan Nox | Search & Destroy | pam", "mp_toujane", "sd", 8, 12, 34, "pam", "cl"),
    ("192.0.2.31", "Iron Wolves Public Server", "mp_carentan", "war", 14, 20, 58, "", "us"),
    ("198.51.100.7", "Feldgrau Deutsch | TDM 24/7", "mp_burgundy", "war", 9, 16, 112, "", "de"),
    ("198.51.100.22", "Zombie Farmhouse | Survive the Night", "mp_farmhouse", "dm", 11, 24, 187, "zombie", "br"),
    ("203.0.113.5", "Los Cuervos | CTF | PAM 4.x", "mp_breakout", "ctf", 6, 14, 143, "pam", "es"),
    ("203.0.113.19", "Maquis FR | Recherche et Destruction", "mp_matmata", "sd", 10, 12, 96, "pam", "fr"),
    ("192.0.2.58", "Red Ball Express | Headquarters", "mp_dawnville", "hq", 3, 18, 71, "", "us"),
    ("198.51.100.41", "Pampa Gamers | DM abierto", "mp_toujane", "dm", 5, 16, 29, "", "cl"),
    ("203.0.113.77", "Panzerfaust Alley | Hardcore", "mp_carentan", "war", 12, 20, 208, "hardcore", "de"),
    ("192.0.2.90", "Night Shift Zombies [Co-op]", "mp_burgundy", "dm", 7, 20, 134, "zombie", "br"),
    ("198.51.100.64", "Veteranos SD | Sin lag | PAM", "mp_decoy", "sd", 12, 12, 52, "pam", "es"),
    ("203.0.113.101", "Les Loups de Verdun | TDM", "mp_toujane", "war", 0, 16, 98, "", "fr"),
    ("192.0.2.120", "Cold Steel Gaming | FFA", "mp_railyard", "dm", 2, 14, 163, "", "us"),
]

_COUNTRY = {row[0]: row[8] for row in _SERVERS}

_PLAYERS = [
    ("^1Sgt.Pepper", 42, 31), ("^7Bruma", 37, 45), ("^2Kestrel", 31, 28),
    ("^5Tovarich", 24, 77), ("^7Mosquito", 19, 52), ("^3Hans_G", 15, 90),
    ("^7Capi", 9, 38), ("^4Lobo", 4, 120),
]


def sample_servers() -> list[ServerInfo]:
    """Servidores de ejemplo de CoD2: nombres de clan inventados, IPs de documentación."""
    servers = []
    for host, name, map_, gt, n, cap, ping, mod, _cc in _SERVERS:
        players = []
        if host == SELECTED_HOST:
            for raw, score, p_ping in _PLAYERS[:n]:
                players.append(Player(_strip(raw), score, p_ping, raw))
        else:
            players = [Player(f"Player{i + 1}", max(0, 20 - 3 * i), 40 + 7 * i)
                       for i in range(n)]
        rules = {"fs_game": f"mods/{mod}" if mod else "", "sv_maxclients": str(cap),
                 "shortversion": "1.3", "g_gametype": gt, "sv_pure": "1",
                 "mapname": map_}
        addr = ServerAddress(host, 28960)
        servers.append(ServerInfo(addr, addr, _strip(name), map_, gt, n, cap, False,
                                  "1.3", ping, players, rules, raw_name=name))
    return servers


def _strip(text: str) -> str:
    out, i = [], 0
    while i < len(text):
        if text[i] == "^" and i + 1 < len(text) and text[i + 1].isdigit():
            i += 2
            continue
        out.append(text[i])
        i += 1
    return "".join(out)


class _FakeCountryDB:
    def lookup(self, ip):
        return _COUNTRY.get(ip)


class _FakeGeo:
    def load(self, path, refresh=False):
        return _FakeCountryDB(), None


class _FakeMaps:
    def get(self, game, install_dir, map_name, allow_download):
        return None


def _fake_scan(servers):
    async def scan(game, masters, on_server, on_status, **kw):
        on_progress = kw.get("on_progress")
        for i, info in enumerate(servers, 1):
            on_server(info)
            if on_progress:
                on_progress(i, len(servers))
        return ScanResult(masters_ok=1, servers_listed=len(servers),
                          servers_responded=len(servers))
    return scan


def _pump(app, seconds: float, until=None) -> None:
    end = time.monotonic() + seconds
    while time.monotonic() < end and not (until and until()):
        app.processEvents()
        time.sleep(0.01)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--lang", choices=("en", "es"), default="en")
    ap.add_argument("--out", type=Path, default=None)
    args = ap.parse_args(argv)
    out = args.out or ROOT / "docs" / ("screenshot.png" if args.lang == "en"
                                      else f"screenshot.{args.lang}.png")

    app = QApplication.instance() or QApplication(sys.argv)
    previous_lang = i18n.current_language()
    i18n.set_language(args.lang)
    servers = sample_servers()
    with tempfile.TemporaryDirectory() as tmp:
        cfg = Config(theme="dark", language=args.lang)
        w = MainWindow(cfg, Path(tmp) / "config.json", scan_fn=_fake_scan(servers),
                       geo=_FakeGeo(), map_service=_FakeMaps(),
                       exe_icon_fn=lambda p: None)
        try:
            w.resize(*SIZE)
            w.show()
            w.select_game(GAME)
            _pump(app, 10, lambda: w.model.rowCount() == len(servers)
                  and w._country_db is not None)
            _expand_tree(w)
            _select_server(w, SELECTED_HOST)
            w.table.setColumnWidth(COL_COUNTRY, 70)
            w.players_table.sortByColumn(1, Qt.DescendingOrder)  # mejores puntajes arriba
            _pump(app, 0.5)
            w.resize(*SIZE)
            _pump(app, 0.3)
            out.parent.mkdir(parents=True, exist_ok=True)
            ok = w.grab().save(str(out), "PNG")
        finally:
            w.close()
            i18n.set_language(previous_lang)
    if not ok:
        print(f"No se pudo guardar {out}", file=sys.stderr)
        return 1
    print(out)
    return 0


def _expand_tree(w: MainWindow) -> None:
    item = w.game_tree.game_item(GAME)
    item.setExpanded(True)
    for i in range(item.childCount()):
        item.child(i).setExpanded(True)


def _select_server(w: MainWindow, host: str) -> None:
    for row in range(w.proxy.rowCount()):
        src = w.proxy.mapToSource(w.proxy.index(row, 0)).row()
        if w.model.server_at(src).address.host == host:
            w.table.selectRow(row)
            return
    raise RuntimeError(f"servidor {host} no encontrado")


if __name__ == "__main__":
    sys.exit(main())
