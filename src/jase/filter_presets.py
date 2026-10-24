"""Filtros guardados: combinaciones con nombre de filtros rápidos + texto del filtro."""
from __future__ import annotations

from dataclasses import asdict, dataclass
from typing import Any

from jase.config import _parse_quick_filters
from jase.models import QuickFilters

MAX_PRESETS = 30


@dataclass(frozen=True)
class FilterPreset:
    name: str
    quick: QuickFilters
    text: str = ""


def parse(raw: Any) -> list[FilterPreset]:
    """Tolerante: lo inválido se descarta; nombres únicos (casefold); como mucho MAX_PRESETS."""
    if not isinstance(raw, list):
        return []
    result: list[FilterPreset] = []
    seen: set[str] = set()
    for item in raw:
        if not isinstance(item, dict):
            continue
        name = item.get("name")
        if not isinstance(name, str) or not name.strip():
            continue
        name = name.strip()
        key = name.casefold()
        if key in seen:
            continue
        quick_raw = item.get("quick")
        quick = _parse_quick_filters(quick_raw if isinstance(quick_raw, dict) else {})
        text = item.get("text")
        seen.add(key)
        result.append(FilterPreset(name, quick, text if isinstance(text, str) else ""))
        if len(result) >= MAX_PRESETS:
            break
    return result


def to_json(presets: list[FilterPreset]) -> list[dict]:
    return [{"name": p.name, "quick": asdict(p.quick), "text": p.text} for p in presets]


def upsert(presets: list[FilterPreset], preset: FilterPreset) -> list[FilterPreset]:
    """Mismo nombre (casefold) reemplaza en su sitio; si no, se añade (tope MAX_PRESETS)."""
    key = preset.name.casefold()
    out = list(presets)
    for i, p in enumerate(out):
        if p.name.casefold() == key:
            out[i] = preset
            return out
    out.append(preset)
    return out[:MAX_PRESETS]


def remove(presets: list[FilterPreset], name: str) -> list[FilterPreset]:
    key = name.casefold()
    return [p for p in presets if p.name.casefold() != key]
