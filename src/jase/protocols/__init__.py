import re

from jase.models import Player

_CONTROL_RE = re.compile(r"[\x00-\x1f\x7f]")
# Bots de Quake 3 / Quake II: ping 0 (ioq3, ET, Urban Terror, Xonotic...), «[BOT]Nombre»
# (Xonotic, mods de CoD4) o «bot123» con ping 999 (los testclients de CoD).
_BOT_TAG_RE = re.compile(r"^\s*\[bot\]", re.IGNORECASE)
_COD_BOT_RE = re.compile(r"^bot\d+$", re.IGNORECASE)


class ProtocolError(Exception):
    """Respuesta de servidor malformada o inesperada."""


def strip_control(text: str) -> str:
    """Quita caracteres de control ASCII (\\x00-\\x1f, \\x7f) de nombres."""
    return _CONTROL_RE.sub("", text)


def is_bot(player: Player) -> bool:
    """Heurística para protocolos sin marca de bot (Quake 3, Quake II), sacada de
    servidores reales: ping 0, etiqueta [BOT] o «botN» con ping 999."""
    name = player.name.strip()
    return (player.ping == 0 or bool(_BOT_TAG_RE.match(name))
            or (player.ping == 999 and bool(_COD_BOT_RE.match(name))))


def count_bots(players: list[Player]) -> int:
    return sum(1 for p in players if is_bot(p))
