"""Historial de ocupación por servidor (lógica pura, sin Qt).

Guarda muestras (instante, jugadores, máximo) de favoritos y recientes en `cache/history.json`
y calcula el promedio de jugadores por hora local. El tiempo siempre se inyecta (`now`).
"""
from __future__ import annotations

import json
from pathlib import Path

from jase.fetch_meta import atomic_write

MIN_GAP_S = 300
KEEP_DAYS = 14
MAX_SAMPLES = 4032
MAX_FILE_BYTES = 8 * 1024 * 1024


class History:
    def __init__(self, path: Path) -> None:
        self.path = Path(path)
        # {game_id: {"ip:puerto": [[t, jugadores, máximo], ...]}} en orden cronológico
        self._data: dict[str, dict[str, list[list[int]]]] = {}

    def record(self, game_id: str, address: str, players: int, max_players: int,
               now: float) -> None:
        samples = self._data.setdefault(game_id, {}).setdefault(str(address), [])
        if samples and now - samples[-1][0] < MIN_GAP_S:
            return
        samples.append([int(now), int(players), int(max_players)])
        cutoff = now - KEEP_DAYS * 86400
        while samples and samples[0][0] < cutoff:
            samples.pop(0)
        if len(samples) > MAX_SAMPLES:
            del samples[:len(samples) - MAX_SAMPLES]

    def samples(self, game_id: str, address: str) -> list[list[int]]:
        return list(self._data.get(game_id, {}).get(str(address), []))

    def hourly_average(self, game_id: str, address: str, now: float,
                       tz_offset_s: int) -> list[float | None]:
        """Promedio de jugadores por hora local (24 valores; None si esa hora no tiene datos)."""
        sums = [0] * 24
        counts = [0] * 24
        cutoff = now - KEEP_DAYS * 86400
        for t, players, _max in self._data.get(game_id, {}).get(str(address), []):
            if t < cutoff:
                continue
            hour = int((t + tz_offset_s) // 3600) % 24
            sums[hour] += players
            counts[hour] += 1
        return [sums[h] / counts[h] if counts[h] else None for h in range(24)]

    def prune(self, keep: set[tuple[str, str]]) -> None:
        """Quita servidores que ya no son favoritos ni recientes."""
        for game_id in list(self._data):
            servers = self._data[game_id]
            for address in list(servers):
                if (game_id, address) not in keep:
                    del servers[address]
            if not servers:
                del self._data[game_id]

    def to_bytes(self) -> bytes:
        return json.dumps(self._data, separators=(",", ":")).encode("utf-8")

    def load(self) -> None:
        """Carga tolerante: ausente, enorme, corrupto o con forma rara → historial vacío."""
        self._data = {}
        try:
            if self.path.stat().st_size > MAX_FILE_BYTES:
                return
            raw = json.loads(self.path.read_bytes())
        except (OSError, ValueError):
            return
        if not isinstance(raw, dict):
            return
        for game_id, servers in raw.items():
            if not isinstance(game_id, str) or not isinstance(servers, dict):
                continue
            for address, samples in servers.items():
                if not isinstance(address, str) or not isinstance(samples, list):
                    continue
                clean = [[s[0], s[1], s[2]] for s in samples
                         if isinstance(s, list) and len(s) == 3
                         and all(isinstance(v, int) and not isinstance(v, bool) for v in s)]
                if clean:
                    self._data.setdefault(game_id, {})[address] = clean[-MAX_SAMPLES:]

    def save(self) -> None:
        atomic_write(self.path, self.to_bytes())
