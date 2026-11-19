"""Genera las capturas de pantalla del README con datos de ejemplo inventados.

Uso: .venv/bin/python scripts/screenshot.py [--lang es|en|pt|fr|de|it] [--theme ID] [--terminal-color C]
    [--out PATH] [--password-dialog | --add-server-dialog | --settings-dialog TAB
    | --find-games-dialog | --now-playing-dialog | --player-search-dialog | --rcon-dialog
    | --friends-dialog | --party-dialog | --context-menu | --about-dialog | --note-dialog
    | --slot-free-dialog]
Corre con Qt offscreen, config temporal y sin red: los servidores, los jugadores
y los países son ficticios (IPs solo de los rangos de documentación RFC 5737).
Con --password-dialog captura solo el diálogo de contraseña del servidor elegido
(guardada «secreto»), y con --add-server-dialog el de «Agregar servidor» con
«connect 192.0.2.14:28960», para compararlos con los del port a Rust. Con
--settings-dialog TAB, Ajustes de Config() en esa pestaña, y con --find-games-dialog,
«Buscar juegos» con lo hallado de FOUND; en ambos solo CoD2 está instalado. Las
herramientas y vigilancias de la 5i, con los datos fijos de `--screenshot` de Rust:
--now-playing-dialog («Ahora se juega» con CoD2, los NOW_SERVERS de CoD1 y un escaneo sin
terminar), --player-search-dialog («jan»), --rcon-dialog («> status» y RCON_ANSWER),
--friends-dialog (FRIENDS) y --party-dialog (el umbral del servidor elegido). Las de la 5j,
del servidor elegido: --context-menu (el menú contextual), --about-dialog («Acerca de»),
--note-dialog («Editar nota…» con NOTE_TEXT) y --slot-free-dialog («Hay sitio»).
"""
from __future__ import annotations

import argparse
import os
import sys
import tempfile
import time
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from PySide6.QtCore import QPoint, QSize, Qt
from PySide6.QtWidgets import QApplication, QInputDialog, QLabel, QMessageBox

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "src"))

from jase import detect, i18n, party  # noqa: E402
from jase.config import Config  # noqa: E402
from jase.models import Player, ServerAddress, ServerInfo  # noqa: E402
from jase.scanner import ScanResult  # noqa: E402
from jase.ui.add_server import AddServerDialog  # noqa: E402
from jase.ui.dialogs import PasswordDialog, SettingsDialog  # noqa: E402
from jase.i18n import tr, tr_n  # noqa: E402
from jase.ui.find_games import FindGamesDialog  # noqa: E402
from jase.ui import icons  # noqa: E402
from jase.ui.friends_dialog import FriendsDialog  # noqa: E402
from jase.ui.main_window import APP_NAME, MainWindow, app_version  # noqa: E402
from jase.ui.now_playing import NowPlayingDialog  # noqa: E402
from jase.ui.player_search import PlayerSearchDialog  # noqa: E402
from jase.ui.rcon_dialog import RconDialog  # noqa: E402
from jase.ui.server_model import COL_COUNTRY  # noqa: E402
from jase.ui.theme import TERMINAL_COLORS, THEME_IDS  # noqa: E402

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

# Lo que halla «Buscar juegos» (FOUND de rust/jase/src/screenshot.rs): juego, ruta, origen.
FOUND = [
    ("cod2", "C:\\Program Files (x86)\\Activision\\Call of Duty 2", "registro"),
    ("cod2", "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Call of Duty 2", "steam"),
    ("cod1", "C:\\Program Files (x86)\\Activision\\Call of Duty", "registro"),
]
SETTINGS_TABS = ("games", "masters", "general", "appearance")
# Lo que trae «Ahora se juega» de CoD1 (NOW_SERVERS de Rust): host, nombre, mapa, jugadores,
# máx, ping. «jan» halla dos.
NOW_GAME = "cod1"
NOW_SERVERS = [
    ("192.0.2.200", "Jan's Brecourt Night", "mp_brecourt", ("Jan", "Ziggy", "Moro"), 16, 61),
    ("203.0.113.150", "Old Guard | Vanilla SD", "mp_harbor", ("Janosch", "Kiwi"), 10, 88),
]
SEARCH_TEXT = "jan"
RCON_ANSWER = """map: mp_toujane
num score ping name            lastmsg address               qport rate
--- ----- ---- --------------- ------- --------------------- ----- -----
  0    42   31 Sgt.Pepper            0 203.0.113.50:28960     4823 25000
  1    37   45 Bruma                50 198.51.100.12:28960    1907 25000
  2    31   28 Kestrel               0 192.0.2.77:28960       3311 25000"""
FRIENDS = ["Bruma", "Jan", "Kestrel"]
NOTE_TEXT = "Clan amigo, martes 21 h"

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


def now_servers() -> list[ServerInfo]:
    """Los NOW_SERVERS de CoD1, en línea."""
    servers = []
    for host, name, map_, names, cap, ping in NOW_SERVERS:
        players = [Player(n, 10 - 3 * i, 50 + 10 * i) for i, n in enumerate(names)]
        addr = ServerAddress(host, 28960)
        servers.append(ServerInfo(addr, addr, name, map_, "sd", len(players), cap, False,
                                  "1.5", ping, players, {}, raw_name=name))
    return servers


def _tool_dialog(args, w: MainWindow, servers: list[ServerInfo]):
    """El diálogo de la 5i que pide `args`, ya armado con los datos fijos."""
    selected = next(s for s in servers if s.address.host == SELECTED_HOST)
    lists = {GAME: servers, NOW_GAME: now_servers()}
    if args.now_playing_dialog:
        dialog = NowPlayingDialog(w)
        dialog.set_lists(lists)
        people = sum(i.humans for found in lists.values() for i in found if i.online)
        # Como `_now_show` con CoD:UO aún escaneando.
        dialog.set_status(tr_n("{n} jugador en tus juegos", "{n} jugadores en tus juegos",
                               people) + " · " + tr_n("escaneando 1 juego…",
                                                      "escaneando {n} juegos…", 1))
        dialog.set_busy(True)
    elif args.player_search_dialog:
        dialog = PlayerSearchDialog(lambda: lists, w)
        dialog.query_edit.setText(SEARCH_TEXT)
        dialog.refresh()
    elif args.rcon_dialog:
        dialog = RconDialog(selected.name, str(selected.connect_address), "secreto", w)
        dialog.command_edit.setText("status")
        dialog._send()
        dialog.show_answer(RCON_ANSWER)
    elif args.friends_dialog:
        dialog = FriendsDialog(FRIENDS, True, w)
    else:
        # `edit_party_alert`: el `QInputDialog.getInt` armado a mano para poder capturarlo.
        dialog = QInputDialog(w)
        dialog.setWindowTitle(tr("Avisar cuando se arme partida"))
        dialog.setLabelText(tr("Avisar cuando {server} tenga al menos estos jugadores "
                               "(0 = no avisar):", server=selected.name))
        dialog.setInputMode(QInputDialog.InputMode.IntInput)
        dialog.setIntRange(0, party.MAX_THRESHOLD)
        dialog.setIntValue(party.DEFAULT_THRESHOLD)
    return dialog


def _parity_dialog(args, w: MainWindow, servers: list[ServerInfo]):
    """El diálogo de la 5j que pide `args`, armado a mano como lo abre la app (sin `exec`)."""
    selected = next(s for s in servers if s.address.host == SELECTED_HOST)
    if args.about_dialog:
        # `QMessageBox.about`: el icono de la app (el de `__main__`) a 64 px.
        dialog = QMessageBox(QMessageBox.Icon.Information,
                             tr("Acerca de {name}", name=APP_NAME),
                             f"{APP_NAME}\n" + tr("Versión {version}", version=app_version()),
                             QMessageBox.StandardButton.Ok, w)
        icon = icons.app_icon()
        dialog.setIconPixmap(icon.pixmap(icon.actualSize(QSize(64, 64))))
    elif args.note_dialog:
        # `edit_note_selected`: el `QInputDialog.getText`.
        dialog = QInputDialog(w)
        dialog.setWindowTitle(tr("Editar nota…"))
        dialog.setLabelText(tr("Nota para {address}", address=selected.address))
        dialog.setTextValue(NOTE_TEXT)
    else:
        # `_on_slot_free` → `confirm_plain(…, Yes)`.
        dialog = QMessageBox(QMessageBox.Icon.Question, tr("Hay sitio"),
                             tr("Hay sitio en {name} ({players}/{max}). ¿Conectar ahora?",
                                name=selected.name, players=selected.num_players,
                                max=selected.max_players),
                             QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No, w)
        dialog.setTextFormat(Qt.TextFormat.PlainText)
        dialog.setDefaultButton(QMessageBox.StandardButton.Yes)
    return dialog


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


def _fake_find(**_kw) -> dict[str, list[detect.Finding]]:
    found: dict[str, list[detect.Finding]] = {}
    for game_id, path, source in FOUND:
        found.setdefault(game_id, []).append(detect.Finding(game_id, path, source))
    return found


def _only_cod2(game, install_dir):
    """Sin disco: solo CoD2 está instalado (el `game_installed` de las capturas de Rust)."""
    return game.exe_relpath if game.id == GAME else None


def _pump(app, seconds: float, until=None) -> None:
    end = time.monotonic() + seconds
    while time.monotonic() < end and not (until and until()):
        app.processEvents()
        time.sleep(0.01)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--lang", choices=i18n.LANGUAGES, default="en")
    ap.add_argument("--theme", choices=("system", *THEME_IDS), default="dark",
                    help="tema de la captura (las del README van en dark)")
    ap.add_argument("--terminal-color", choices=TERMINAL_COLORS, default="green",
                    help="color del fósforo con --theme terminal")
    ap.add_argument("--out", type=Path, default=None)
    dialogs = ap.add_mutually_exclusive_group()
    dialogs.add_argument("--password-dialog", action="store_true",
                         help="captura el diálogo de contraseña en vez de la ventana")
    dialogs.add_argument("--add-server-dialog", action="store_true",
                         help="captura el diálogo «Agregar servidor» en vez de la ventana")
    dialogs.add_argument("--settings-dialog", choices=SETTINGS_TABS,
                         help="captura Ajustes en esa pestaña en vez de la ventana")
    dialogs.add_argument("--find-games-dialog", action="store_true",
                         help="captura «Buscar juegos» en vez de la ventana")
    for flag, what in (("--now-playing-dialog", "«Ahora se juega»"),
                       ("--player-search-dialog", "«Buscar jugador» con «jan»"),
                       ("--rcon-dialog", "la consola RCON tras «status»"),
                       ("--friends-dialog", "«Amigos»"),
                       ("--party-dialog", "el umbral de partida"),
                       ("--context-menu", "el menú contextual del elegido"),
                       ("--about-dialog", "«Acerca de»"),
                       ("--note-dialog", "«Editar nota…» del elegido"),
                       ("--slot-free-dialog", "«Hay sitio» del elegido")):
        dialogs.add_argument(flag, action="store_true",
                             help=f"captura {what} en vez de la ventana")
    args = ap.parse_args(argv)
    out = args.out or ROOT / "docs" / ("screenshot.png" if args.lang == "en"
                                      else f"screenshot.{args.lang}.png")

    app = QApplication.instance() or QApplication(sys.argv)
    previous_lang = i18n.current_language()
    i18n.set_language(args.lang)
    servers = sample_servers()
    with tempfile.TemporaryDirectory() as tmp:
        cfg = Config(theme=args.theme, terminal_color=args.terminal_color, language=args.lang)
        w = MainWindow(cfg, Path(tmp) / "config.json", scan_fn=_fake_scan(servers),
                       geo=_FakeGeo(), map_service=_FakeMaps(),
                       exe_icon_fn=lambda p: None)
        try:
            w.resize(*SIZE)
            w.show()
            w.select_game(GAME)
            _pump(app, 10, lambda: w.model.rowCount() == len(servers)
                  and w._country_db is not None)
            w.body_splitter.setSizes([300, SIZE[0] - 300])
            _expand_tree(w)
            _select_server(w, SELECTED_HOST)
            w.table.setColumnWidth(COL_COUNTRY, 70)
            w.players_table.sortByColumn(1, Qt.DescendingOrder)  # mejores puntajes arriba
            _pump(app, 0.5)
            w.resize(*SIZE)
            _pump(app, 0.3)
            out.parent.mkdir(parents=True, exist_ok=True)
            if args.settings_dialog or args.find_games_dialog:
                detect.resolve_game_exe = _only_cod2
                if args.settings_dialog:
                    dialog = SettingsDialog(Config(), w, on_find_games=dict)
                    dialog.tabs.setCurrentIndex(SETTINGS_TABS.index(args.settings_dialog))
                else:
                    dialog = FindGamesDialog(Config(), w, find_fn=_fake_find)
                dialog.show()
                searching = i18n.tr("Buscando…")
                _pump(app, 5, lambda: all(label.text() != searching
                                          for label in dialog.findChildren(QLabel)))
                _pump(app, 0.3)
                ok = dialog.grab().save(str(out), "PNG")
                dialog.close()
            elif (args.now_playing_dialog or args.player_search_dialog or args.rcon_dialog
                  or args.friends_dialog or args.party_dialog):
                dialog = _tool_dialog(args, w, servers)
                dialog.show()
                _pump(app, 0.5)
                ok = dialog.grab().save(str(out), "PNG")
                dialog.close()
            elif args.context_menu:
                # `_on_table_context_menu` con el elegido, con `popup` en vez de `exec`.
                menu = w._build_context_menu()
                menu.popup(w.table.viewport().mapToGlobal(QPoint(0, 0)))
                _pump(app, 0.5)
                ok = menu.grab().save(str(out), "PNG")
                menu.close()
            elif args.about_dialog or args.note_dialog or args.slot_free_dialog:
                dialog = _parity_dialog(args, w, servers)
                dialog.show()
                _pump(app, 0.5)
                ok = dialog.grab().save(str(out), "PNG")
                dialog.close()
            elif args.password_dialog or args.add_server_dialog:
                if args.password_dialog:
                    name = next(s.name for s in servers if s.address.host == SELECTED_HOST)
                    dialog = PasswordDialog(name, "secreto", w)
                else:
                    dialog = AddServerDialog(w.game_ids, GAME, False,
                                             f"connect {SELECTED_HOST}:28960",
                                             detect=lambda *a: None, parent=w)
                dialog.show()
                _pump(app, 0.3)
                ok = dialog.grab().save(str(out), "PNG")
                dialog.close()
            else:
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
