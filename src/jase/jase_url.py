"""Enlaces `jase://connect/<juego>/<host>:<puerto>`: construcción y análisis estricto.

El enlace llega de fuera (navegador, chat): `parse` rechaza todo lo que no sea exactamente esa
forma. Nunca se conecta solo; la ventana pide confirmación al usuario.
"""
from __future__ import annotations

import re

from jase.games import GAMES
from jase.masters import valid_host
from jase.models import ServerAddress

SCHEME = "jase"
ACTION = "connect"
MAX_LENGTH = 300

_GAME_RE = re.compile(r"[a-z0-9_]{1,32}")
_PORT_RE = re.compile(r"[0-9]{1,5}")


def build(game_id: str, address: ServerAddress) -> str:
    return f"{SCHEME}://{ACTION}/{game_id}/{address.host}:{address.port}"


def is_link(text: str) -> bool:
    return text[:len(SCHEME) + 3].lower() == f"{SCHEME}://"


def parse(text: str) -> tuple[str, ServerAddress] | None:
    if not isinstance(text, str) or not text or len(text) > MAX_LENGTH:
        return None
    if not text.isascii() or not text.isprintable() or " " in text:
        return None
    if not is_link(text):
        return None
    rest = text[len(SCHEME) + 3:]
    if any(c in rest for c in "?#@\\%"):
        return None
    if rest.endswith("/"):
        rest = rest[:-1]
    parts = rest.split("/")
    if len(parts) != 3 or parts[0].lower() != ACTION:
        return None
    game_id, target = parts[1], parts[2]
    if not _GAME_RE.fullmatch(game_id) or game_id not in GAMES:
        return None
    host, sep, port_text = target.rpartition(":")
    if not sep or not valid_host(host) or host[0] in ".-" or not _PORT_RE.fullmatch(port_text):
        return None
    port = int(port_text)
    if not 1 <= port <= 65535:
        return None
    return game_id, ServerAddress(host, port)
