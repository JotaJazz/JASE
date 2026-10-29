"""API web de Steam: IGameServersService/GetServerList (lista de servidores con clave).

Sustituye al master UDP de Valve en los juegos cuyo master ya no responde. La
respuesta es JSON: `{"response": {"servers": [{"addr": "ip:puerto", ...}]}}`."""
from __future__ import annotations

import json
from urllib.parse import urlencode

from jase.models import ServerAddress
from jase.protocols import ProtocolError

# Único host al que se envía la clave: nunca se toma de masters.json ni de un master propio.
OFFICIAL_HOST = "api.steampowered.com"
API_URL = f"https://{OFFICIAL_HOST}/IGameServersService/GetServerList/v1/"


class SteamWebError(ProtocolError):
    """Fallo de la API web con un mensaje ya traducido y sin datos sensibles.

    `kind` es estable (ver scanner.MasterError): "missing_key", "invalid_key",
    "rate_limited" o "unsupported_host"."""

    def __init__(self, message: str, kind: str) -> None:
        super().__init__(message)
        self.kind = kind


def build_url(host: str, key: str, filter: str, limit: int = 20000) -> str:
    if host.lower() != OFFICIAL_HOST:
        raise ValueError("La clave de Steam solo se envía a " + OFFICIAL_HOST)
    query = urlencode({"key": key, "filter": filter, "limit": limit})
    return API_URL + "?" + query


def parse_server_list(data: bytes) -> list[ServerAddress]:
    return parse_response(data)[0]


def parse_response(data: bytes) -> tuple[list[ServerAddress], int]:
    """(direcciones sin repetir, entradas de la respuesta): las entradas dicen si se
    llegó al tope de la API aunque haya direcciones repetidas."""
    try:
        response = json.loads(data)["response"]
        if not isinstance(response, dict):
            raise TypeError("response")
    except (ValueError, KeyError, TypeError, RecursionError) as exc:
        raise ProtocolError("Respuesta de la API web de Steam inválida") from exc
    servers: dict[ServerAddress, None] = {}
    entries = response.get("servers") or []
    for entry in entries:
        host, sep, port = str(entry.get("addr", "")).rpartition(":") if isinstance(entry, dict) \
            else ("", "", "")
        if sep and host and port.isdigit() and 0 < int(port) < 65536:
            servers[ServerAddress(host, int(port))] = None
    return list(servers), len(entries) if isinstance(entries, list) else 0
