"""API web de Steam: IGameServersService/GetServerList (lista de servidores con clave).

Sustituye al master UDP de Valve en los juegos cuyo master ya no responde. La
respuesta es JSON: `{"response": {"servers": [{"addr": "ip:puerto", ...}]}}`."""
from __future__ import annotations

import json
from urllib.parse import urlencode

from jase.models import ServerAddress
from jase.protocols import ProtocolError

API_URL = "https://{host}/IGameServersService/GetServerList/v1/"


class SteamWebError(ProtocolError):
    """Fallo de la API web con un mensaje ya traducido y sin datos sensibles."""


def build_url(host: str, key: str, filter: str, limit: int = 20000) -> str:
    query = urlencode({"key": key, "filter": filter, "limit": limit})
    return API_URL.format(host=host) + "?" + query


def parse_server_list(data: bytes) -> list[ServerAddress]:
    try:
        response = json.loads(data)["response"]
        if not isinstance(response, dict):
            raise TypeError("response")
    except (ValueError, KeyError, TypeError) as exc:
        raise ProtocolError("Respuesta de la API web de Steam inválida") from exc
    servers: dict[ServerAddress, None] = {}
    for entry in response.get("servers") or []:
        host, sep, port = str(entry.get("addr", "")).rpartition(":") if isinstance(entry, dict) \
            else ("", "", "")
        if sep and host and port.isdigit() and 0 < int(port) < 65536:
            servers[ServerAddress(host, int(port))] = None
    return list(servers)
