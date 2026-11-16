"""Genera los golden JSON de los parsers (versión Python) para el test de paridad de Rust.

Uso: python scripts/protocol_golden.py  (escribe rust/jase-core/tests/golden/*.json)

Cada fixture de tests/fixtures/ pasa por el parser de su familia con las opciones por
defecto y su resultado se escribe en JSON; rust/jase-core/tests/golden.rs exige lo mismo.
Un fixture que no esté en la tabla ni en EXCLUDED hace salir con 1.
"""
from __future__ import annotations

import dataclasses
import fnmatch
import json
import sys
from collections.abc import Callable
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "src"))

from jase.models import Player, ServerAddress, ServerInfo  # noqa: E402
from jase.protocols import ProtocolError  # noqa: E402
from jase.protocols import gamespy, quake2, quake3, steamweb, ut2004, valve, zandronum  # noqa: E402

FIXTURES = ROOT / "tests" / "fixtures"
OUT = ROOT / "rust" / "jase-core" / "tests" / "golden"
ADDR = ServerAddress("192.0.2.1", 27960)

Q3_MASTERS = ("cod1", "cod2", "cod4", "coduo", "et", "jka", "oa", "q3", "rtcw", "sof2",
              "urt", "xonotic")
Q3_STATUS = ("cod2x", "cod4", "coduo", "et", "jk2", "jka", "oa", "q3", "rtcw",
             "smokinguns", "sof2", "tremulous", "urt", "warfork", "warsow", "xonotic")
# Lo cubre el port de tests/test_ut2004.py (varios mensajes con marco).
EXCLUDED = ("ut2004_master_1.bin",)


def _gamespy_status(data: bytes) -> ServerInfo:
    asm = gamespy.StatusAssembler()
    asm.feed(data)
    return gamespy.parse_status(asm.fields(), ADDR, 1)


TABLE: list[tuple[tuple[str, ...], Callable[[bytes], Any]]] = [
    (tuple(f"{g}_master_*.bin" for g in Q3_MASTERS), quake3.parse_getservers_response),
    (tuple(f"{g}_status*.bin" for g in Q3_STATUS) + ("oa_master_mixed_baseq3_status.bin",),
     lambda d: quake3.parse_status_response(d, ADDR, 1)),
    (("q2_master_1.bin",), quake2.parse_master_response),
    (("q2_status.bin", "dday_status.bin"), lambda d: quake2.parse_status(d, ADDR, 1)),
    (("bf1942_master_openspy.bin", "mohaa*_master_333.bin"), gamespy.parse_master_list),
    (("bf1942_status.bin", "mohaa*_status*.bin", "to*_status.bin"), _gamespy_status),
    (("cs16_master_gamedir_1.bin",), valve.parse_master_response),
    (("cs16_info*.bin",), lambda d: valve.parse_info(d, ADDR, 1)),
    (("cs16_player.bin",), valve.parse_players),
    (("cs16_player_challenge.bin",), valve.challenge_of),
    (("ut2004_info.bin",), lambda d: ut2004.parse_info(d, ADDR, 1)),
    (("ut2004_players.bin",), ut2004.parse_players),
    (("zandronum_master.bin",), zandronum.parse_master_packet),
    (("zandronum_players.bin", "zandronum_team.bin"),
     lambda d: zandronum.parse_response(d, ADDR, 1)),
    (("steamweb_l4d2.json",), steamweb.parse_response),
]


def to_json(value: Any) -> Any:
    """Forma común con tests/golden.rs (ver to_json allí)."""
    if isinstance(value, ServerAddress):
        return str(value)
    if isinstance(value, Player):
        return {"name": value.name, "score": value.score, "ping": value.ping,
                "raw_name": value.raw_name}
    if isinstance(value, ServerInfo):
        return {f.name: to_json(getattr(value, f.name)) for f in dataclasses.fields(value)}
    if isinstance(value, (list, tuple)):
        return [to_json(v) for v in value]
    if isinstance(value, bytes):
        return list(value)
    if isinstance(value, dict):
        return {k: to_json(v) for k, v in value.items()}
    return value


def parser_for(name: str) -> Callable[[bytes], Any] | None:
    for patterns, parse in TABLE:
        if any(fnmatch.fnmatchcase(name, p) for p in patterns):
            return parse
    return None


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    missing = []
    for path in sorted(FIXTURES.iterdir()):
        if path.name in EXCLUDED:
            continue
        parse = parser_for(path.name)
        if parse is None:
            missing.append(path.name)
            continue
        try:
            result = to_json(parse(path.read_bytes()))
        except ProtocolError:
            result = {"error": True}
        text = json.dumps(result, ensure_ascii=False, indent=1) + "\n"
        (OUT / f"{path.name}.json").write_text(text, encoding="utf-8")
    if missing:
        print("Fixtures sin parser en la tabla:", ", ".join(missing), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
