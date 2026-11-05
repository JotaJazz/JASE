# Protocolos (familias)

Cada `GameDef.family` elige cómo se pide la lista al master y cómo se consulta cada
servidor. Parsers en `src/jase/protocols/`, red en `src/jase/scanner.py`
(`fetch_master`, `_query_one`). Capturas reales en `tests/fixtures/*.bin`.

| Familia | Master | Consulta | Juegos |
|---|---|---|---|
| `quake3` | UDP `getservers <protocolo> full empty` (con `gamename` en dpmaster); varios paquetes, fin `\EOT`/`\EOF` | UDP `getstatus` → `statusResponse` | 17: CoD 1/UO/2/4, Q3, RtCW, ET, UrT, OpenArena, JK2, JKA, SoF2, Xonotic, Warsow, Warfork, Tremulous, Smokin' Guns |
| `quake2` | UDP `query` → `servers` + entradas de 6 bytes, sin marca de fin (se acumula hasta el plazo) | UDP `status` → `print` | Quake II, D-Day |
| `gamespy` | TCP 28900: `\secure\` → `validate` (`gsmsalg`) + `\list\` | UDP `\status\` en varios paquetes (`queryid`); puerto de juego en `hostport` o consulta − 1 | UT99, Tactical Ops, MoHAA (3), BF1942 |
| `valve` | UDP `0x31` por páginas (máx. 20), o API web de Steam con clave | A2S_INFO (con challenge) y A2S_PLAYER; multipaquete GoldSrc/Source | CS 1.6, L4D, L4D2, CS:S, TF2, HLDM, DoD, TFC, GMod |
| `ut2004` | TCP con hashes de CD key ficticios fijos (nunca reales) | UDP al puerto + 1: info, reglas, jugadores | UT2004 |
| `zandronum` | UDP, paquetes numerados con Huffman | UDP con Huffman; un servidor ignora 10 s tras otra consulta (se reintenta una vez) | Zandronum |

## Reglas comunes

- Un parser recibe bytes y devuelve datos o lanza `ProtocolError`. Nada de red.
- Números del servidor recortados a 32 bits (`bounded_int`); nombres sin caracteres de
  control (`strip_control`); colores de Quake (`^N`) aparte en `raw_name`.
- `ServerInfo.address` = dirección de **consulta** (la que guardan Favoritos y
  Recientes); `connect_address` = la de **conexión** (puede diferir: GameSpy, UT2004,
  EDF de Valve).
- Masters que mezclan juegos: se separan con `status_gamename`, `status_gamedir`,
  `valve_gamedir`, `status_exclude_mods` o `list_filter` (Tactical Ops en la lista de UT99).

## Casos especiales

- **Steam (API web):** `fetch_master_steamweb`. La clave solo se envía a
  `api.steampowered.com`; las IPs con 100 servidores o más y los nombres «This is a
  dummy…» se descartan como spam (hasta 6 pasadas excluyéndolas).
- **Windows:** `SIO_UDP_CONNRESET` se desactiva en el socket compartido
  (`_disable_udp_connreset`) para que un ICMP no lo deje sordo.
- **Mismo servidor con dos nombres** (favorito por nombre, reciente por IP): una
  consulta a la vez por IP:puerto (`_Router.locks`).
- **LAN** (`lan.py`): difusión de la consulta de la familia a los puertos típicos del
  juego; luego `query_servers` normal.
- **Detectar juego** (`identify.py`): consulta con cada familia y cruza la respuesta con
  los `GameDef` (Zandronum primero: ignora consultas seguidas).
- **RCON** (`rcon.py`): Quake (UDP), GoldSrc (UDP con challenge) y Source (TCP).

Masters en uso y descartados en los sondeos: README, secciones «Masters used» y
«Discarded in probing».
