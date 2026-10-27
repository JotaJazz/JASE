"""«Se armó partida»: avisar cuando un servidor vigilado llega a N jugadores (sin Qt).

`Config.party_alerts` guarda {juego: {"host:puerto": umbral}}. Cuentan solo las personas
(`ServerInfo.humans`): un servidor lleno de bots no es una partida.
"""
from __future__ import annotations

from typing import Any

from jase.models import ServerInfo

MAX_THRESHOLD = 128
DEFAULT_THRESHOLD = 4


def parse(raw: Any) -> dict[str, dict[str, int]]:
    """Tolerante: juego por juego y entrada por entrada; lo inválido se descarta."""
    if not isinstance(raw, dict):
        return {}
    result: dict[str, dict[str, int]] = {}
    for game_id, alerts in raw.items():
        if not isinstance(game_id, str) or not isinstance(alerts, dict):
            continue
        clean = {a: n for a, n in alerts.items()
                 if isinstance(a, str) and ":" in a and isinstance(n, int)
                 and not isinstance(n, bool) and 1 <= n <= MAX_THRESHOLD}
        if clean:
            result[game_id] = clean
    return result


class PartyTracker:
    """Decide cuándo avisar: al cruzar el umbral hacia arriba, una vez. Se vuelve a armar
    cuando el servidor baja a la mitad del umbral (así 4→3→4 no avisa dos veces). La
    primera respuesta solo fija el punto de partida: abrir JASE no avisa de golpe."""

    def __init__(self) -> None:
        self._armed: dict[tuple[str, str], bool] = {}

    def reset(self, game_id: str, address: str) -> None:
        self._armed.pop((game_id, address), None)

    def update(self, game_id: str, info: ServerInfo, threshold: int) -> bool:
        if not info.online:
            return False
        key = (game_id, str(info.address))
        humans = info.humans
        armed = self._armed.get(key)
        if armed is None:
            self._armed[key] = humans < threshold
            return False
        if armed and humans >= threshold:
            self._armed[key] = False
            return True
        if not armed and humans <= threshold // 2:
            self._armed[key] = True
        return False
