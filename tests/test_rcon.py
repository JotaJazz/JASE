"""RCON: los tres protocolos contra servidores falsos locales y la consola en la ventana."""
import asyncio
import socket
import struct
import threading

import pytest

from jase import passwords, rcon
from jase.config import Config
from jase.games import GAMES
from jase.models import ServerAddress, ServerInfo


def udp_server(replies):
    """Servidor UDP en un hilo: a cada datagrama recibido le contesta con `replies(data)`
    (lista de datagramas). Devuelve (dirección, recibidos, hilo, socket)."""
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind(("127.0.0.1", 0))
    sock.settimeout(1.5)
    got = []

    def run():
        while True:
            try:
                data, addr = sock.recvfrom(4096)
            except OSError:
                return
            got.append(data)
            for reply in replies(data):
                sock.sendto(reply, addr)

    thread = threading.Thread(target=run, daemon=True)
    thread.start()
    return ServerAddress("127.0.0.1", sock.getsockname()[1]), got, thread, sock


def test_kind_of_each_family():
    assert rcon.kind_of(GAMES["cod2"]) == "quake" and rcon.kind_of(GAMES["q2"]) == "quake"
    assert rcon.kind_of(GAMES["cs16"]) == "goldsrc"
    assert rcon.kind_of(GAMES["tf2"]) == "source"
    assert rcon.kind_of(GAMES["ut99"]) is None and rcon.kind_of(GAMES["zandronum"]) is None


def test_clean_command_is_one_line():
    assert rcon.clean_command(" kick a\nquit ") == "kick a quit"
    assert len(rcon.clean_command("x" * 5000)) == rcon.MAX_COMMAND


def test_quake_rcon_joins_print_packets_and_strips_colors():
    addr, got, thread, sock = udp_server(
        lambda d: [rcon.OOB + b"print\n^1map: ^7mp_toujane\n", rcon.OOB + b"print\nnum score\n"])
    try:
        text = asyncio.run(rcon.quake(addr, "secreta", "status", timeout=1))
    finally:
        sock.close()
    assert got[0] == rcon.OOB + b"rcon secreta status"
    assert text == "map: mp_toujane\nnum score\n"


def test_quake_rcon_bad_password_and_timeout():
    addr, _got, _t, sock = udp_server(lambda d: [rcon.OOB + b"print\nBad rconpassword.\n"])
    try:
        with pytest.raises(rcon.RconError) as err:
            asyncio.run(rcon.quake(addr, "x", "status", timeout=1))
    finally:
        sock.close()
    assert err.value.code == "bad_password"
    addr, _got, _t, sock = udp_server(lambda d: [])
    try:
        with pytest.raises(rcon.RconError) as err:
            asyncio.run(rcon.quake(addr, "x", "status", timeout=0.3))
    finally:
        sock.close()
    assert err.value.code == "timeout"


def test_goldsrc_challenge_then_command():
    def replies(data):
        if data.startswith(rcon.OOB + b"challenge rcon"):
            return [rcon.OOB + b"challenge rcon 4242\n"]
        return [rcon.OOB + b"lhostname: Pampa CS\n"]
    addr, got, _t, sock = udp_server(replies)
    try:
        text = asyncio.run(rcon.goldsrc(addr, "clave", "status", timeout=1))
    finally:
        sock.close()
    assert got[1] == rcon.OOB + b'rcon 4242 "clave" status\n'
    assert text == "hostname: Pampa CS\n"


def test_source_rcon_auth_and_multi_packet_answer():
    async def run():
        received = []

        async def handle(reader, writer):
            async def read():
                (size,) = struct.unpack("<i", await reader.readexactly(4))
                data = await reader.readexactly(size)
                rid, kind = struct.unpack_from("<ii", data)
                return rid, kind, data[8:-2].decode()
            rid, kind, body = await read()
            received.append((kind, body))
            writer.write(rcon.source_packet(rid, 0, "")
                         + rcon.source_packet(rid if body == "buena" else -1, 2, ""))
            if body != "buena":
                await writer.drain()
                writer.close()
                return
            rid, kind, body = await read()
            received.append((kind, body))
            writer.write(rcon.source_packet(rid, 0, "hostname: TF2 ")
                         + rcon.source_packet(rid, 0, "Pampa\n"))
            rid, _kind, _body = await read()  # el paquete vacío que marca el final
            writer.write(rcon.source_packet(rid, 0, ""))
            await writer.drain()
            writer.close()

        server = await asyncio.start_server(handle, "127.0.0.1", 0)
        port = server.sockets[0].getsockname()[1]
        addr = ServerAddress("127.0.0.1", port)
        try:
            text = await rcon.source(addr, "buena", "status", timeout=2)
            with pytest.raises(rcon.RconError) as err:
                await rcon.source(addr, "mala", "status", timeout=2)
        finally:
            server.close()
            await server.wait_closed()
        return text, received, err.value.code

    text, received, code = asyncio.run(run())
    assert text == "hostname: TF2 Pampa\n"
    assert received[:2] == [(3, "buena"), (2, "status")] and code == "bad_password"


def test_send_rejects_unsupported_games():
    with pytest.raises(rcon.RconError) as err:
        asyncio.run(rcon.send(GAMES["ut99"], ServerAddress("1.1.1.1", 7778), "x", "status"))
    assert err.value.code == "unsupported"


def test_window_console_sends_and_remembers_password(qtbot, tmp_path):
    from jase.ui.main_window import MainWindow
    address = ServerAddress("1.2.3.4", 28960)
    sent = []

    async def fake_send(game, addr, password, command):
        sent.append((game.id, addr, password, command))
        if password != "buena":
            raise rcon.RconError("bad_password")
        return "map: mp_toujane\n"

    async def scan(*a, **k):
        return None
    w = MainWindow(Config(), tmp_path / "c.json", scan_fn=scan)
    qtbot.addWidget(w)
    w._rcon_send_fn = fake_send
    w.current_game = "cod2"
    assert not w.can_rcon()
    w.model.add_or_update(ServerInfo(address, address, "Clan Nox", "m", "dm", 1, 8, False,
                                     "1.3", 20))
    w.table.selectRow(0)
    assert w.ui_actions["rcon"].isEnabled()
    w.ui_actions["rcon"].trigger()
    dialog = next(iter(w._rcon_dialogs.values()))[0]
    assert dialog.windowTitle() == "Consola RCON: Clan Nox"
    dialog.password_edit.setText("mala")
    dialog.command_edit.setText("status")
    dialog.send_button.click()
    qtbot.waitUntil(lambda: dialog.send_button.isEnabled())
    assert "Clave RCON incorrecta." in dialog.output.toPlainText()
    dialog.password_edit.setText("buena")
    dialog.remember_check.setChecked(True)
    dialog.command_edit.setText("status")
    dialog.command_edit.returnPressed.emit()
    qtbot.waitUntil(lambda: "mp_toujane" in dialog.output.toPlainText())
    assert sent[-1] == ("cod2", address, "buena", "status")
    assert passwords.get_rcon_password("cod2", address) == "buena"
    dialog.close()
    assert w._rcon_dialogs == {}
