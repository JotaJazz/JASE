"""Consultas sin interfaz gráfica: `jase-cli games` y `jase-cli scan <juego>`.

Solo usa el núcleo (sin Qt, keyring ni Pillow). Con `--json` la salida sigue el contrato
versionado de docs/cli.md (`"schema": SCHEMA`); un cambio incompatible sube SCHEMA.
Códigos de salida: 0 = algún master respondió (o se agotó --timeout con resultados
parciales), 1 = ningún master respondió, 2 = uso incorrecto.
"""
from __future__ import annotations

import argparse
import asyncio
import json
import os
import sys
import time
from collections.abc import Callable, Mapping
from importlib import metadata
from pathlib import Path
from typing import IO, Any, NoReturn

from jase import geo, i18n, masters
from jase.browse import BASE_MOD, mod_of
from jase.config import default_config_path
from jase.games import GAMES, GameDef
from jase.i18n import tr
from jase.models import ServerInfo
from jase.scanner import MasterError, ScanResult, scan

SCHEMA = 1
STEAM_KEY_ENV = "JASE_STEAM_KEY"
CACHE_DIR_ENV = "JASE_CACHE_DIR"


class _UsageError(Exception):
    pass


def _parser_class(stdout: IO[str], stderr: IO[str]) -> type[argparse.ArgumentParser]:
    """argparse que escribe en los flujos dados y no termina el proceso en un error."""

    class Parser(argparse.ArgumentParser):
        def _print_message(self, message: str, file: object = None) -> None:
            if message:
                (stderr if file is sys.stderr else stdout).write(message)

        def error(self, message: str) -> NoReturn:
            raise _UsageError(message)

    return Parser


def _version() -> str:
    try:
        return metadata.version("jase")
    except metadata.PackageNotFoundError:
        return "unknown"


def _master_arg(value: str) -> tuple[str, int]:
    host, sep, port = value.rpartition(":")
    if not sep or not masters.valid_host(host) or not port.isdigit() \
            or not masters.valid_port(int(port)):
        raise argparse.ArgumentTypeError(
            tr("Master inválido: {value} (se espera host:puerto)", value=value))
    return host, int(port)


def _positive(value: str) -> float:
    try:
        seconds = float(value)
    except ValueError:
        seconds = 0.0
    if seconds <= 0:
        raise argparse.ArgumentTypeError(tr("Se espera un número de segundos mayor que 0"))
    return seconds


def _build_parser(stdout: IO[str], stderr: IO[str]) -> argparse.ArgumentParser:
    parser_class = _parser_class(stdout, stderr)
    parser = parser_class(prog="jase-cli",
                          description=tr("Consulta servidores de juegos clásicos sin interfaz "
                                         "gráfica."))
    parser.add_argument("--version", action="version", version=f"jase-cli {_version()}")
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--json", action="store_true",
                        help=tr("Salida JSON (contrato versionado, ver docs/cli.md)"))
    common.add_argument("--cache-dir", type=Path,
                        help=tr("Carpeta de caché: lista de masters y base de países "
                                "(o variable JASE_CACHE_DIR)"))
    common.add_argument("--no-update", action="store_true",
                        help=tr("No descargar la lista oficial de masters (se revisa como "
                                "mucho una vez al día)"))
    sub = parser.add_subparsers(dest="command", required=True, parser_class=parser_class)
    sub.add_parser("games", parents=[common], help=tr("Lista los juegos soportados"))
    p = sub.add_parser("scan", parents=[common],
                       help=tr("Consulta los masters y los servidores de un juego"))
    p.add_argument("game", help=tr("id del juego (ver «jase-cli games»)"))
    p.add_argument("--steam-key", default=None,
                   help=tr("Clave de la API web de Steam (o variable JASE_STEAM_KEY)"))
    p.add_argument("--include-empty", action="store_true",
                   help=tr("Incluir servidores vacíos de Steam"))
    p.add_argument("--timeout", type=_positive, default=None,
                   help=tr("Tope de tiempo total en segundos (por defecto, sin tope)"))
    p.add_argument("--country", action="store_true",
                   help=tr("Agregar el país de cada servidor (descarga la base de DB-IP "
                           "la primera vez)"))
    p.add_argument("--master", type=_master_arg, action="append", default=None,
                   metavar="HOST:PORT",
                   help=tr("Master a consultar en vez de los oficiales (repetible)"))
    return parser


def _official(cache_dir: Path, update: bool,
              fetch: Callable[[str], bytes]) -> masters.OfficialList:
    if update and masters.should_check(cache_dir / masters.META_FILENAME, time.time()):
        cache_dir.mkdir(parents=True, exist_ok=True)
        masters.check_official(cache_dir, time.time(), fetch)
    return masters.load_official(cache_dir)


def _games_json(official: masters.OfficialList) -> dict[str, Any]:
    return {"schema": SCHEMA, "jase": _version(), "games": [
        {"id": g.id, "name": g.name, "short_name": g.short_name or g.name,
         "family": g.family, "needs_steam_key": g.steam_webapi,
         "uses_steam_api": g.uses_steam_api,
         "masters": [{"host": h, "port": p} for h, p in masters.official_for(g.id, official)]}
        for g in GAMES.values()]}


def _error_message(error: MasterError) -> str:
    """Los mensajes de la clave de Steam de la app hablan de Ajustes; aquí, de la CLI."""
    if error.kind == "missing_key":
        return tr("Falta la clave de API de Steam; usa --steam-key o la variable "
                  "JASE_STEAM_KEY")
    if error.kind == "invalid_key":
        return tr("La clave de API de Steam no es válida")
    return error.message


def _server_json(game: GameDef, info: ServerInfo, country: str | None) -> dict[str, Any]:
    mod = mod_of(game, info)
    return {
        "address": str(info.address),
        "connect": str(info.connect_address),
        "connect_port_deduced": info.connect_port_deduced,
        "name": info.name,
        "map": info.map,
        "gametype": info.gametype,
        "mod": None if mod == BASE_MOD else mod,
        "players": info.num_players,
        "max_players": info.max_players,
        "bots": info.bots,
        "humans": info.humans,
        "password": info.password,
        "version": info.version,
        "ping_ms": info.ping_ms,
        "country": country,
        "player_list": [{"name": p.name, "score": p.score, "ping": p.ping}
                        for p in info.players],
        "rules": dict(info.rules),
    }


def _country_lookup(source, cache_dir: Path, stderr: IO[str]) -> Callable[[str], str | None]:
    db, error = source.load(cache_dir / geo.DB_FILENAME)
    if db is None:
        stderr.write(f"jase-cli: {error}\n")
        return lambda host: None

    def lookup(host: str) -> str | None:
        code = db.lookup(host)
        return code.lower() if code else None
    return lookup


def _run_scan(args, game: GameDef, official: masters.OfficialList, scan_fn,
              environ: Mapping[str, str], stdout: IO[str], stderr: IO[str],
              country_source) -> int:
    cache_dir: Path = args.cache_dir
    master_list = args.master or list(masters.official_for(game.id, official))
    country_of = (_country_lookup(country_source or geo.CountrySource(), cache_dir, stderr)
                  if args.country else (lambda host: None))
    steam_key = args.steam_key if args.steam_key is not None else environ.get(STEAM_KEY_ENV, "")
    servers: list[ServerInfo] = []
    started = time.monotonic()

    async def main() -> ScanResult | None:
        coro = scan_fn(game, master_list, servers.append, lambda status: None,
                       steam_key=steam_key, steam_include_empty=args.include_empty)
        try:
            return await asyncio.wait_for(coro, args.timeout)
        except TimeoutError:
            return None

    result = asyncio.run(main())
    timed_out = result is None
    errors = [] if result is None else list(result.errors)
    failed = {e.master for e in errors}
    data = {
        "schema": SCHEMA,
        "jase": _version(),
        "game": {"id": game.id, "name": game.name},
        "duration_s": round(time.monotonic() - started, 3),
        "timed_out": timed_out,
        "masters": [{"host": h, "port": p, "ok": None if timed_out else f"{h}:{p}" not in failed}
                    for h, p in master_list],
        "masters_ok": None if result is None else result.masters_ok,
        "errors": [{"master": e.master, "kind": e.kind, "message": _error_message(e)}
                   for e in errors],
        "servers_listed": None if result is None else result.servers_listed,
        "servers_responded": len(servers),
        "servers": [_server_json(game, info, country_of(info.address.host))
                    for info in servers],
    }
    if args.json:
        json.dump(data, stdout, ensure_ascii=False, indent=1)
        stdout.write("\n")
    else:
        _print_table(data, stdout, stderr)
    return 1 if result is not None and not result.ok else 0


def _print_table(data: dict[str, Any], stdout: IO[str], stderr: IO[str]) -> None:
    for e in data["errors"]:
        stderr.write(f"{e['master']}: {e['message']}\n")
    if data["timed_out"]:
        stderr.write(tr("Se alcanzó el tope de tiempo; resultados parciales") + "\n")
    rows = sorted(data["servers"], key=lambda s: (-s["players"], s["ping_ms"]))
    for s in rows:
        country = f"{s['country'] or '--':2}  " if any(r["country"] for r in rows) else ""
        stdout.write(f"{s['connect']:<22}  {country}{s['players']:>3}/{s['max_players']:<3}  "
                     f"{s['ping_ms']:>4} ms  {s['map'][:20]:<20}  {s['name']}\n")
    if data["servers_listed"] is not None:
        stderr.write(tr("{responded} de {listed} servidores respondieron",
                        responded=data["servers_responded"],
                        listed=data["servers_listed"]) + "\n")


def main(argv: list[str] | None = None, *, scan_fn=scan, stdout: IO[str] | None = None,
         stderr: IO[str] | None = None, environ: Mapping[str, str] | None = None,
         country_source=None, masters_fetch: Callable[[str], bytes] = masters.default_fetch,
         ) -> int:
    stdout = stdout or sys.stdout
    stderr = stderr or sys.stderr
    environ = os.environ if environ is None else environ
    i18n.set_language(i18n.environment_language(environ))
    parser = _build_parser(stdout, stderr)
    try:
        args = parser.parse_args(sys.argv[1:] if argv is None else argv)
        game = None
        if args.command == "scan":
            game = GAMES.get(args.game)
            if game is None:
                raise _UsageError(tr("Juego desconocido: {id}", id=args.game))
    except _UsageError as exc:
        stderr.write(f"jase-cli: {exc}\n")
        return 2
    except SystemExit as exc:  # --help / --version
        return int(exc.code or 0)
    if args.cache_dir is None:
        args.cache_dir = Path(environ.get(CACHE_DIR_ENV) or default_config_path().parent / "cache")
    official = _official(args.cache_dir, not args.no_update, masters_fetch)
    if game is None:
        if args.json:
            json.dump(_games_json(official), stdout, ensure_ascii=False, indent=1)
            stdout.write("\n")
        else:
            for g in GAMES.values():
                key = ("  [Steam key]" if g.steam_webapi
                       else "  [Steam key optional]" if g.uses_steam_api else "")
                stdout.write(f"{g.id:<10}  {g.name}{key}\n")
        return 0
    return _run_scan(args, game, official, scan_fn, environ, stdout, stderr, country_source)


if __name__ == "__main__":
    sys.exit(main())
