from __future__ import annotations

import tempfile
from pathlib import Path

from PySide6.QtCore import Qt
from PySide6.QtGui import QColor

from jase.i18n import tr

# Gráficos del estilo Windows 98 (flechas, ✓, trama del scroll).
WIN98_DIR = Path(__file__).resolve().parent.parent / "assets" / "win98"
# Gráficos del estilo Luna (✓ verde, estrías del scroll).
LUNA_DIR = WIN98_DIR.parent / "luna"
# Flechas y tramas teñidas con el color de cada tema (se generan al construir la hoja).
# La ventana fija la carpeta en la caché del usuario (set_cache_dir); sin ella (tests,
# capturas) se usa una carpeta temporal privada del proceso.
_cache_dir: Path | None = None
_ARROWS = {  # dirección → (ancho, alto, trazado)
    "up": (9, 5, "M0 5L4.5 0L9 5Z"),
    "down": (9, 5, "M0 0H9L4.5 5Z"),
    "right": (5, 9, "M0 0L5 4.5L0 9Z"),
}

THEME_IDS: tuple[str, ...] = (
    "dark", "light", "neon", "brown", "green", "classic", "xp", "terminal", "synthwave",
    "rust", "goldsrc",
)
# Terminal elige el color del fósforo aparte (Config.terminal_color); cada color es una
# variante con su propia paleta: "terminal_<color>".
TERMINAL_COLORS: tuple[str, ...] = ("green", "white", "sepia", "amber")

_BADGES_DARK = {
    "badge_cod1": "#936b09", "badge_coduo": "#8a6d3b", "badge_cod2": "#c0562b",
    "badge_cod4": "#4b7a3c", "badge_q3": "#c54c1b", "badge_rtcw": "#7a5c8a",
    "badge_et": "#3b6ea5", "badge_jka": "#3b78a8", "badge_sof2": "#a04040",
    "badge_cs16": "#a65a17", "badge_ut99": "#8b701b", "badge_tacops": "#66733a", "badge_mohaa": "#5a7a4a",
    "badge_mohaas": "#8a4b3c", "badge_mohaab": "#3c7a7a", "badge_bf1942": "#5e7a2c",
    "badge_urt": "#4f6f3a",
    "badge_oa": "#8a6d1f",
    "badge_q2": "#a0522d",
    "badge_ut2004": "#6a5a9a",
    "badge_zandronum": "#9a2a1f",
    "badge_l4d": "#a03a3a",
    "badge_l4d2": "#8a4040",
    "badge_css": "#3b78a8",
    "badge_tf2": "#a65a17",
    "badge_hldm": "#a65a17",
    "badge_dod": "#5e7a2c",
    "badge_tfc": "#8a4b3c",
    "badge_jk2": "#4a6a9a",
    "badge_xonotic": "#2f7a6e",
    "badge_warsow": "#9a4a6a",
    "badge_warfork": "#7a4a9a",
    "badge_tremulous": "#6a7a2a",
    "badge_smokinguns": "#9a5a2a",
    "badge_dday": "#6b6b3a",
    "badge_gmod": "#2f6fa0",
}
_BADGES_LIGHT = {
    "badge_cod1": "#8a6408", "badge_coduo": "#67512c", "badge_cod2": "#904020",
    "badge_cod4": "#385b2d", "badge_q3": "#a23e16", "badge_rtcw": "#5b4567",
    "badge_et": "#2c527b", "badge_jka": "#2c5b81", "badge_sof2": "#8a3333",
    "badge_cs16": "#8a4d17", "badge_ut99": "#8a6f1b", "badge_tacops": "#566132", "badge_mohaa": "#435b37",
    "badge_mohaas": "#67382d", "badge_mohaab": "#2d5b5b", "badge_bf1942": "#475c21",
    "badge_urt": "#3e5a2e",
    "badge_oa": "#7a5f18",
    "badge_q2": "#8a4524",
    "badge_ut2004": "#55487b",
    "badge_zandronum": "#82231a",
    "badge_l4d": "#8a3030",
    "badge_l4d2": "#783838",
    "badge_css": "#2c5b81",
    "badge_tf2": "#8a4d17",
    "badge_hldm": "#8a4d17",
    "badge_dod": "#475c21",
    "badge_tfc": "#67382d",
    "badge_jk2": "#37507a",
    "badge_xonotic": "#235c53",
    "badge_warsow": "#7a3853",
    "badge_warfork": "#5c387a",
    "badge_tremulous": "#4f5b1f",
    "badge_smokinguns": "#7a4520",
    "badge_dday": "#52522c",
    "badge_gmod": "#245580",
}

TOKENS: dict[str, dict[str, str]] = {
    "dark": {
        "bg": "#16181d", "surface": "#1d2027", "sidebar": "#1a1d23", "border": "#2a2e37",
        "text": "#d7dbe2", "muted": "#7c8494", "accent": "#3b82f6", "selection": "#24324a",
        "selection_text": "#d7dbe2",
        "ping_good": "#3ecf6e", "ping_mid": "#e5b93a", "ping_bad": "#ef5a5a",
        "occupancy": "#3b82f6", "favorite": "#f5c542",
        "bevel_light": "#2a2e37", "bevel_dark": "#2a2e37",
        "button_top": "#1d2027", "button_bottom": "#1d2027",
        **_BADGES_DARK,
    },
    "light": {
        "bg": "#f6f7f9", "surface": "#ffffff", "sidebar": "#eef0f3", "border": "#e5e7eb",
        "text": "#1f2328", "muted": "#6b7280", "accent": "#2563eb", "selection": "#dbeafe",
        "selection_text": "#1f2328",
        "ping_good": "#15803d", "ping_mid": "#a16207", "ping_bad": "#b91c1c",
        "occupancy": "#60a5fa", "favorite": "#b7791f",
        "bevel_light": "#e5e7eb", "bevel_dark": "#e5e7eb",
        "button_top": "#ffffff", "button_bottom": "#ffffff",
        **_BADGES_LIGHT,
    },
    "neon": {
        "bg": "#0b0f14", "surface": "#0f161d", "sidebar": "#0d131a", "border": "#19f5e1",
        "text": "#c8fff9", "muted": "#6fa8a3", "accent": "#19f5e1", "selection": "#0e2f33",
        "selection_text": "#19f5e1",
        "ping_good": "#19f5e1", "ping_mid": "#ffd23f", "ping_bad": "#ff5c7a",
        "occupancy": "#19f5e1", "favorite": "#ffd23f",
        "bevel_light": "#19f5e1", "bevel_dark": "#19f5e1",
        "button_top": "#0f161d", "button_bottom": "#0f161d",
        **_BADGES_DARK,
    },
    "brown": {
        "bg": "#2b211a", "surface": "#342820", "sidebar": "#32261d", "border": "#4a3a2c",
        "text": "#ecdcc8", "muted": "#a8927a", "accent": "#c08a4a", "selection": "#5a3f28",
        "selection_text": "#fff3e0",
        "ping_good": "#8fcf6a", "ping_mid": "#e0b060", "ping_bad": "#ef6a5a",
        "occupancy": "#c08a4a", "favorite": "#f0c060",
        "bevel_light": "#4a3a2c", "bevel_dark": "#4a3a2c",
        "button_top": "#3a2c21", "button_bottom": "#3a2c21",
        **_BADGES_DARK,
    },
    "green": {
        "bg": "#151c14", "surface": "#1c2619", "sidebar": "#182116", "border": "#2c3a28",
        "text": "#cfe8c4", "muted": "#86a07c", "accent": "#6aa84f", "selection": "#2f4a26",
        "selection_text": "#e6f8de",
        "ping_good": "#8fd16a", "ping_mid": "#e0c050", "ping_bad": "#ef6a5a",
        "occupancy": "#6aa84f", "favorite": "#e0c050",
        "bevel_light": "#2c3a28", "bevel_dark": "#2c3a28",
        "button_top": "#1c2619", "button_bottom": "#1c2619",
        **_BADGES_DARK,
    },
    "classic": {
        "bg": "#c0c0c0", "surface": "#ffffff", "sidebar": "#c0c0c0", "border": "#808080",
        "text": "#000000", "muted": "#404040", "accent": "#000080", "selection": "#000080",
        "selection_text": "#ffffff",
        "ping_good": "#006400", "ping_mid": "#8a5a00", "ping_bad": "#b00000",
        "occupancy": "#000080", "favorite": "#8a5a00",
        "bevel_light": "#ffffff", "bevel_dark": "#404040",
        "button_top": "#c0c0c0", "button_bottom": "#c0c0c0",
        **_BADGES_LIGHT,
    },
    "xp": {
        "bg": "#ece9d8", "surface": "#ffffff", "sidebar": "#ece9d8", "border": "#aca899",
        "text": "#000000", "muted": "#555555", "accent": "#316ac5", "selection": "#316ac5",
        "selection_text": "#ffffff",
        "ping_good": "#007000", "ping_mid": "#8a5a00", "ping_bad": "#b00000",
        "occupancy": "#316ac5", "favorite": "#8a5a00",
        "bevel_light": "#ffffff", "bevel_dark": "#aca899",
        "button_top": "#ffffff", "button_bottom": "#d6d0c0",
        **_BADGES_LIGHT,
    },
    # Terminal: fósforo verde, blanco y ámbar sobre negro; sepia como papel de teletipo.
    "terminal_green": {
        "bg": "#050a05", "surface": "#081008", "sidebar": "#081008", "border": "#1d6b2c",
        "text": "#33ff66", "muted": "#22b34a", "accent": "#7dff9e", "selection": "#33ff66",
        "selection_text": "#050a05",
        "ping_good": "#33ff66", "ping_mid": "#e6ff4d", "ping_bad": "#ff6a4a",
        "occupancy": "#33ff66", "favorite": "#e6ff4d",
        "bevel_light": "#1d6b2c", "bevel_dark": "#1d6b2c",
        "button_top": "#081008", "button_bottom": "#081008",
        **_BADGES_DARK,
    },
    "terminal_white": {
        "bg": "#0a0a0a", "surface": "#0e0e0e", "sidebar": "#0e0e0e", "border": "#5a5a5a",
        "text": "#e8e8e8", "muted": "#a0a0a0", "accent": "#ffffff", "selection": "#e8e8e8",
        "selection_text": "#0a0a0a",
        "ping_good": "#9ee89e", "ping_mid": "#f0d070", "ping_bad": "#ff7a6a",
        "occupancy": "#e8e8e8", "favorite": "#f0d070",
        "bevel_light": "#5a5a5a", "bevel_dark": "#5a5a5a",
        "button_top": "#0e0e0e", "button_bottom": "#0e0e0e",
        **_BADGES_DARK,
    },
    "terminal_sepia": {
        "bg": "#efe3c8", "surface": "#f6ecd6", "sidebar": "#f6ecd6", "border": "#9c8462",
        "text": "#3b2a1a", "muted": "#6e5940", "accent": "#5a3d22", "selection": "#3b2a1a",
        "selection_text": "#f6ecd6",
        "ping_good": "#2f6a1f", "ping_mid": "#7a5200", "ping_bad": "#a32a1a",
        "occupancy": "#3b2a1a", "favorite": "#7a5200",
        "bevel_light": "#9c8462", "bevel_dark": "#9c8462",
        "button_top": "#f6ecd6", "button_bottom": "#f6ecd6",
        **_BADGES_LIGHT,
    },
    "terminal_amber": {
        "bg": "#120c02", "surface": "#160f03", "sidebar": "#160f03", "border": "#5a3a00",
        "text": "#ffb000", "muted": "#b07a10", "accent": "#ffd060", "selection": "#ffb000",
        "selection_text": "#120c02",
        "ping_good": "#b8e060", "ping_mid": "#ffd060", "ping_bad": "#ff6a3a",
        "occupancy": "#ffb000", "favorite": "#ffd060",
        "bevel_light": "#5a3a00", "bevel_dark": "#5a3a00",
        "button_top": "#160f03", "button_bottom": "#160f03",
        **_BADGES_DARK,
    },
    "synthwave": {
        "bg": "#1a1033", "surface": "#211442", "sidebar": "#1f1240", "border": "#ff3fa4",
        "text": "#f3e6ff", "muted": "#a58bd0", "accent": "#ff3fa4", "selection": "#4a1f5e",
        "selection_text": "#ffffff",
        "ping_good": "#3ff7ff", "ping_mid": "#ffe14a", "ping_bad": "#ff5c8a",
        "occupancy": "#ff3fa4", "favorite": "#ffe14a",
        "bevel_light": "#ff3fa4", "bevel_dark": "#ff3fa4",
        "button_top": "#2b1055", "button_bottom": "#2b1055",
        **_BADGES_DARK,
    },
    "rust": {
        "bg": "#1c1410", "surface": "#22170f", "sidebar": "#2a1a12", "border": "#6b2e1a",
        "text": "#e6d2c0", "muted": "#a08670", "accent": "#b5451b", "selection": "#7a2a14",
        "selection_text": "#fff0e6",
        "ping_good": "#8fc860", "ping_mid": "#e88a3a", "ping_bad": "#ef5a4a",
        "occupancy": "#b5451b", "favorite": "#e8a43a",
        "bevel_light": "#6b2e1a", "bevel_dark": "#6b2e1a",
        "button_top": "#2a1a12", "button_bottom": "#2a1a12",
        **_BADGES_DARK,
    },
    # VGUI de Steam en la era GoldSrc (buscador de servidores de Half-Life / CS 1.6).
    "goldsrc": {
        "bg": "#4c5844", "surface": "#3e4637", "sidebar": "#4c5844", "border": "#292c21",
        "text": "#d8ded3", "muted": "#a0aa95", "accent": "#c4b550", "selection": "#80752a",
        "selection_text": "#ffffff",
        "ping_good": "#a6d96a", "ping_mid": "#e8c860", "ping_bad": "#ef6a5a",
        "occupancy": "#958831", "favorite": "#e8c860",
        "bevel_light": "#8c9284", "bevel_dark": "#292c21",
        "button_top": "#4c5844", "button_bottom": "#4c5844",
        **_BADGES_DARK,
    },
}

# Separadores internos (rejilla, divisores) y contorno de la fila seleccionada.
# Igual que border / selection salvo en neon y synthwave, donde el borde es de neón.
for _tid, _t in TOKENS.items():
    _t["divider"] = {"neon": "#0f4d49", "synthwave": "#4a1f5e"}.get(_tid, _t["border"])
    _t["selection_outline"] = {"neon": "#19f5e1", "synthwave": "#ff3fa4"}.get(_tid, _t["selection"])
    # Borde de campos, listas y botones: el azul acero de Luna en XP.
    _t["field_border"] = {"xp": "#7f9db9"}.get(_tid, _t["border"])
    # Luz interior del biselado hundido: se ve también sobre fondo blanco (98 usa #dfdfdf).
    _t["bevel_inner"] = {"classic": "#dfdfdf"}.get(_tid, _t["bevel_light"])
    # Fondo de los tooltips: el amarillo pálido de 98 en Clásico.
    _t["tooltip"] = {"classic": "#ffffe1", "xp": "#ffffe1"}.get(_tid, _t["surface"])

# Colores propios de Luna (XP): clave → (valor en XP, token del que copia el resto).
_LUNA_TOKENS = {
    "button_border": ("#003c74", "field_border"),   # azul oscuro de los botones
    "hover": ("#f8b330", "accent"),                 # brillo naranja al pasar el mouse
    "hover_light": ("#ffe7a2", "accent"),
    "default_top": ("#cee7ff", "button_top"),       # brillo azul del botón por defecto
    "default_bottom": ("#6982ee", "button_bottom"),
    "pressed_top": ("#cdcac3", "button_bottom"),
    "pressed_bottom": ("#f1f1ed", "button_top"),
    "disabled_bg": ("#f5f4ea", "bg"),
    "disabled_border": ("#c9c7ba", "border"),
    "disabled_text": ("#aca899", "muted"),
    "scroll_top": ("#d6e2fd", "button_top"),        # pulgar y flechas celestes del scroll
    "scroll_bottom": ("#b6cbf7", "button_bottom"),
    "scroll_border": ("#93a9e0", "border"),
    "scroll_track": ("#f4f3ee", "bg"),
    "check": ("#21a121", "accent"),                 # ✓ verde de las casillas
    "check_border": ("#1c5180", "field_border"),
    "check_top": ("#dcdcd7", "surface"),
    "header_bg": ("#ebeadb", "sidebar"),
    "header_line": ("#d6d2c2", "divider"),
    "pane": ("#fcfcfe", "surface"),                 # interior de las pestañas
    "tab_border": ("#919b9c", "border"),
    "tree_line": ("#a0a0a0", "border"),             # líneas punteadas del árbol
    "tree_box": ("#7898b5", "border"),
    "tree_box_bottom": ("#d4d0c8", "surface"),
    "progress": ("#2fbd2f", "selection"),           # bloques verdes de progreso
    "progress_light": ("#a3eda3", "selection"),
    "progress_border": ("#a0a0a0", "border"),
}
for _tid, _t in TOKENS.items():
    for _key, (_xp, _base) in _LUNA_TOKENS.items():
        _t[_key] = _xp if _tid == "xp" else _t[_base]
# Personalidad de los temas planos: clave → (valores por tema, token del que copia el resto).
_FLAVOR_TOKENS = {
    "arrow": ({"neon": "#19f5e1"}, "muted"),                         # flechas de árbol/combo
    "header_text": ({"neon": "#19f5e1", "green": "#8fd16a", "synthwave": "#ff8c42"}, "muted"),
    "bar_top": ({"synthwave": "#3a1466", "brown": "#46342a"}, "bg"),  # degradado de la barra
    "sunset": ({"synthwave": "#ff8c42"}, "accent"),                  # naranja del atardecer
    # Barrido CRT de Terminal (sepia es papel: sin barrido).
    "scanline": ({f"terminal_{c}": "#000000" for c in ("green", "white", "amber")}, "bg"),
}
for _tid, _t in TOKENS.items():
    for _key, (_values, _base) in _FLAVOR_TOKENS.items():
        _t[_key] = _values.get(_tid, _t[_base])

# Terminal: flechas del árbol en el color del fósforo.
for _color in TERMINAL_COLORS:
    TOKENS[f"terminal_{_color}"]["arrow"] = TOKENS[f"terminal_{_color}"]["text"]

# Borde de casillas: en Oscuro/Claro el de siempre casi no se ve sobre el fondo.
TOKENS["dark"]["check_border"] = "#5b6372"
TOKENS["light"]["check_border"] = "#9ca3af"

# Ids con paleta propia: los temas elegibles, con Terminal desdoblado por color.
VARIANT_IDS: tuple[str, ...] = tuple(TOKENS)

# Estilo de widgets por tema: "flat" (redondeado), "win98" (biselado), "luna" (Windows XP),
# "vgui" (Steam de la era GoldSrc: biselados de 1 px sobre oliva) o "terminal" (MS-DOS/Unix).
STYLE: dict[str, str] = {tid: "flat" for tid in VARIANT_IDS}
STYLE["classic"] = "win98"
STYLE["xp"] = "luna"
STYLE["goldsrc"] = "vgui"
for _color in TERMINAL_COLORS:
    STYLE[f"terminal_{_color}"] = "terminal"

# Fuente por tema; None = la de siempre.
FONTS: dict[str, str | None] = {tid: None for tid in VARIANT_IDS}
FONTS["classic"] = 'Tahoma, "MS Sans Serif", "Segoe UI", sans-serif'
FONTS["xp"] = 'Tahoma, "MS Sans Serif", "Segoe UI", sans-serif'
FONTS["goldsrc"] = 'Tahoma, Verdana, "Segoe UI", sans-serif'
for _color in TERMINAL_COLORS:
    FONTS[f"terminal_{_color}"] = ('"Lucida Console", Consolas, "Courier New", '
                                   '"DejaVu Sans Mono", monospace')

# Colores ^0-^9 de Quake 3 / CoD ajustados por tema para que se lean sobre el fondo.
# None = color de texto normal (el blanco ^7 en ambos temas).
_Q3_DARK: dict[str, str | None] = {
    "0": "#6b7280", "1": "#ef5a5a", "2": "#3ecf6e", "3": "#e5c07b", "4": "#5b8def",
    "5": "#56c8d8", "6": "#c678dd", "7": None, "8": "#f0913a", "9": "#8a919e",
}
_Q3_LIGHT: dict[str, str | None] = {
    "0": "#111827", "1": "#c62828", "2": "#15803d", "3": "#a16207", "4": "#1d4ed8",
    "5": "#0e7490", "6": "#9333ea", "7": None, "8": "#c2410c", "9": "#6b7280",
}
Q3_COLORS: dict[str, dict[str, str | None]] = {
    "dark": _Q3_DARK,
    "light": _Q3_LIGHT,
    "neon": dict(_Q3_DARK),
    "brown": {**_Q3_DARK, "0": "#7c8494"},
    "green": dict(_Q3_DARK),
    "classic": dict(_Q3_LIGHT),
    "xp": dict(_Q3_LIGHT),
    "synthwave": dict(_Q3_DARK),
    "rust": dict(_Q3_DARK),
    "goldsrc": {**_Q3_DARK, "0": "#9aa08f", "1": "#ff6b6b", "4": "#7fa8ff", "9": "#b8bfae"},
    # Terminal es monocromo: todo en el color del fósforo.
    **{f"terminal_{c}": dict.fromkeys(_Q3_DARK) for c in TERMINAL_COLORS},
}

# Opacidad de la barra de ocupación (columna Jugadores) sobre el fondo de la celda.
OCCUPANCY_ALPHA = 0.35

PING_GOOD_MAX = 80
PING_MID_MAX = 150


def resolve_theme(pref: str, scheme: Qt.ColorScheme, terminal_color: str = "green") -> str:
    """Id de tema concreto: "system" (o un id inválido) sigue el esquema del sistema;
    "terminal" se vuelve "terminal_<color>" (verde si el color no es válido)."""
    if pref == "terminal":
        return f"terminal_{terminal_color if terminal_color in TERMINAL_COLORS else 'green'}"
    if pref in TOKENS:
        return pref
    return "light" if scheme == Qt.ColorScheme.Light else "dark"


def theme_display_name(theme_id: str) -> str:
    names = {
        "system": tr("Sistema"),
        "dark": tr("Oscuro"),
        "light": tr("Claro"),
        "neon": tr("Neón"),
        "brown": tr("Marrón"),
        "green": tr("Verde"),
        "classic": tr("Clásico"),
        "xp": tr("Windows XP"),
        "terminal": tr("Terminal"),
        "synthwave": tr("Synthwave"),
        "rust": tr("Óxido"),
        "goldsrc": tr("GoldSrc"),
    }
    if theme_id.startswith("terminal_"):
        theme_id = "terminal"
    return names.get(theme_id, theme_id)


def terminal_color_name(color: str) -> str:
    names = {"green": tr("Verde"), "white": tr("Blanco"), "sepia": tr("Sepia"),
             "amber": tr("Ámbar")}
    return names.get(color, color)


def _luminance(hex_color: str) -> float:
    def lin(c: int) -> float:
        v = c / 255
        return v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4
    r, g, b = (int(hex_color[i:i + 2], 16) for i in (1, 3, 5))
    return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)


def contrast_ratio(fg: str, bg: str) -> float:
    """Contraste WCAG entre dos colores "#rrggbb" (1.0 a 21.0)."""
    a, b = _luminance(fg), _luminance(bg)
    hi, lo = max(a, b), min(a, b)
    return (hi + 0.05) / (lo + 0.05)


def ping_color(ping_ms: int, theme_id: str) -> QColor:
    t = TOKENS[theme_id]
    if ping_ms < PING_GOOD_MAX:
        return QColor(t["ping_good"])
    if ping_ms < PING_MID_MAX:
        return QColor(t["ping_mid"])
    return QColor(t["ping_bad"])


# Contraste mínimo del texto de color (ping, ^N) sobre la fila seleccionada.
SELECTION_COLOR_MIN_CONTRAST = 3.5


def legible(color: QColor, background: QColor, fallback: QColor,
            minimum: float = SELECTION_COLOR_MIN_CONTRAST) -> QColor:
    """`color` si se lee sobre `background` (contraste >= `minimum`); si no, `fallback`.

    Sirve para el texto de color (ping, ^N) en filas seleccionadas con selección fuerte
    (azul marino de Clásico, ámbar sólido): ahí pasa al color de texto de la selección.
    """
    if contrast_ratio(color.name(), background.name()) >= minimum:
        return color
    return fallback


def build_stylesheet(theme_id: str) -> str:
    """Hoja de estilos de la app para el tema `theme_id` (id concreto, no "system")."""
    t = TOKENS[theme_id]
    style = STYLE[theme_id]
    radius = {"flat": 6, "win98": 0, "luna": 3, "vgui": 0, "terminal": 0}[style]
    small_radius = {"flat": 4, "win98": 0, "luna": 3, "vgui": 0, "terminal": 0}[style]
    css = f"""
QMainWindow, QWidget {{ background: {t['bg']}; color: {t['text']}; }}
QTableView {{
    background: {t['surface']}; alternate-background-color: {t['surface']};
    border: 1px solid {t['border']}; border-radius: {radius}px;
    gridline-color: {t['divider']};
    selection-background-color: {t['selection']}; selection-color: {t['selection_text']};
}}
QHeaderView::section {{
    background: {t['sidebar']}; color: {t['muted']};
    border: none; border-bottom: 1px solid {t['divider']}; padding: 6px 8px;
}}
QListWidget, QTreeWidget#game_tree {{
    background: {t['sidebar']}; border: 1px solid {t['border']}; border-radius: {radius}px;
    outline: none;
}}
QListWidget::item, QTreeWidget#game_tree::item {{ padding: 6px 8px; border-radius: {radius}px; }}
QListWidget::item:selected, QTreeWidget#game_tree::item:selected {{ background: {t['selection']}; color: {t['selection_text']}; }}
QLineEdit, QPlainTextEdit, QComboBox {{
    background: {t['surface']}; color: {t['text']};
    border: 1px solid {t['border']}; border-radius: {radius}px; padding: 5px 8px;
}}
QLineEdit:focus, QPlainTextEdit:focus, QComboBox:focus {{ border-color: {t['accent']}; }}
QComboBox:disabled, QCheckBox:disabled {{ color: {t['muted']}; }}
QComboBox QAbstractItemView {{
    background: {t['surface']}; color: {t['text']};
    selection-background-color: {t['selection']}; selection-color: {t['selection_text']};
}}
QPushButton {{
    background: {t['surface']}; color: {t['text']};
    border: 1px solid {t['border']}; border-radius: {radius}px; padding: 6px 14px;
}}
QToolBar#toolbar {{ background: {t['bg']}; border: none; spacing: 4px; padding: 4px 8px; }}
QToolBar#toolbar::separator {{ background: {t['divider']}; width: 1px; margin: 4px 6px; }}
QToolButton {{ border: 1px solid transparent; border-radius: {radius}px; padding: 4px; }}
QToolButton:hover {{ background: {t['surface']}; border-color: {t['border']}; }}
QToolButton:pressed {{ background: {t['selection']}; }}
QToolButton::menu-indicator {{ image: none; width: 0px; }}
QMenuBar {{ background: {t['bg']}; color: {t['text']}; }}
QMenuBar::item:selected {{ background: {t['selection']}; color: {t['selection_text']}; }}
QMenu {{ background: {t['surface']}; color: {t['text']}; border: 1px solid {t['border']}; }}
QMenu::item {{ padding: 5px 24px 5px 24px; }}
QMenu::item:selected {{ background: {t['selection']}; color: {t['selection_text']}; }}
QMenu::item:disabled {{ color: {t['muted']}; }}
QMenu::separator {{ height: 1px; background: {t['divider']}; margin: 4px 8px; }}
QFrame#server_card {{
    background: {t['surface']}; border: 1px solid {t['border']}; border-radius: {radius}px;
}}
QFrame#server_card QLabel, QWidget#card_details {{ background: transparent; }}
QFrame#server_card QLabel[role="caption"], QLabel#card_empty, QLabel#card_lock {{
    color: {t['muted']};
}}
QFrame#server_card QLabel#map_image {{
    background: {t['surface']}; color: {t['muted']};
    border: 1px solid {t['border']}; border-radius: {small_radius}px;
}}
QFrame#server_card QLabel#map_image[placeholder="false"] {{ border: none; }}
QPushButton#copy_address {{ padding: 2px 10px; }}
QPushButton#favorite_toggle {{ padding: 0px 6px; color: {t['muted']}; }}
QPushButton#favorite_toggle[favorite="true"] {{ color: {t['favorite']}; }}
"""
    if t["selection_outline"] != t["selection"]:
        css += _outline_rules(t)
    css += _indicator_rules(t, style)
    if style == "flat":
        css += _flat_rules(t) + _flavor_rules(theme_id, t)
    if style == "win98":
        css += _win98_rules(t)
    elif style == "luna":
        css += _luna_rules(t)
    elif style == "vgui":
        css += _vgui_rules(t)
    elif style == "terminal":
        css += _terminal_rules(t)
    css += _font_rules(theme_id)
    return css


def set_cache_dir(path: Path) -> None:
    """Carpeta de los SVG teñidos (p. ej. %APPDATA%\\JimmysASE\\cache\\theme)."""
    global _cache_dir
    _cache_dir = Path(path)


def _tinted_dir() -> Path:
    global _cache_dir
    if _cache_dir is None:
        _cache_dir = Path(tempfile.mkdtemp(prefix="jase-theme-"))
    return _cache_dir


def _tinted_svg(name: str, svg: str) -> str:
    """url() de un SVG generado para el tema; se reescribe solo si cambió su contenido."""
    path = _tinted_dir() / f"{name}.svg"
    try:
        current = path.read_text(encoding="utf-8") if path.is_file() else None
        if current != svg:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(svg, encoding="utf-8")
    except OSError:
        return "none"  # carpeta no escribible: queda el dibujo nativo
    return f'url("{path.as_posix()}")'


def _tinted_arrow(direction: str, color: str) -> str:
    """Flecha (9×5, o 5×9 a la derecha) en `color`."""
    w, h, d = _ARROWS[direction]
    return _tinted_svg(f"arrow_{direction}_{color.lstrip('#')}",
                       f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" '
                       f'viewBox="0 0 {w} {h}"><path d="{d}" fill="{color}"/></svg>')


def _scanlines(color: str) -> str:
    """Trama de líneas de barrido (1 de cada 3 px) en `color` translúcido."""
    return _tinted_svg(f"scanlines_{color.lstrip('#')}",
                       '<svg xmlns="http://www.w3.org/2000/svg" width="4" height="3" '
                       f'viewBox="0 0 4 3" shape-rendering="crispEdges"><rect y="2" width="4" '
                       f'height="1" fill="{color}" fill-opacity="0.35"/></svg>')


def _flat_rules(t: dict[str, str]) -> str:
    """Temas planos: scroll delgado, combos y flechas con color del tema, estados de botón."""
    return f"""
QScrollBar:vertical {{ background: transparent; width: 10px; margin: 2px; border: none; }}
QScrollBar:horizontal {{ background: transparent; height: 10px; margin: 2px; border: none; }}
QScrollBar::handle {{ background: {t['border']}; border-radius: 3px; }}
QScrollBar::handle:hover {{ background: {t['muted']}; }}
QScrollBar::handle:vertical {{ min-height: 24px; }}
QScrollBar::handle:horizontal {{ min-width: 24px; }}
QScrollBar::add-line, QScrollBar::sub-line {{ width: 0px; height: 0px; border: none; }}
QScrollBar::add-page, QScrollBar::sub-page {{ background: none; }}
QAbstractScrollArea::corner {{ background: transparent; border: none; }}
QComboBox::drop-down {{ border: none; width: 20px; }}
QComboBox::down-arrow {{ image: {_tinted_arrow("down", t['arrow'])}; width: 9px; height: 5px; }}
QComboBox::down-arrow:on {{ image: {_tinted_arrow("down", t['accent'])}; }}
QHeaderView::down-arrow {{
    image: {_tinted_arrow("down", t['accent'])}; width: 9px; height: 5px; padding-right: 6px;
}}
QHeaderView::up-arrow {{
    image: {_tinted_arrow("up", t['accent'])}; width: 9px; height: 5px; padding-right: 6px;
}}
QPushButton:hover, QToolButton#presets_button:hover {{ border-color: {t['accent']}; }}
QPushButton:pressed, QToolButton#presets_button:pressed {{ background: {t['selection']}; }}
QPushButton:disabled {{ color: {t['muted']}; background: {t['bg']}; border-color: {t['divider']}; }}
QToolBar#toolbar QToolButton {{ background: transparent; }}
QToolBar#toolbar QToolButton:hover {{ background: {t['surface']}; }}
QToolBar#toolbar QToolButton:pressed {{ background: {t['selection']}; }}
QToolButton#presets_button {{
    background: {t['surface']}; color: {t['text']}; border: 1px solid {t['border']};
    padding: 5px 12px;
}}
QToolTip {{
    background: {t['tooltip']}; color: {t['text']}; border: 1px solid {t['border']};
    padding: 4px 6px;
}}
"""


def _flavor_rules(theme_id: str, t: dict[str, str]) -> str:
    """Detalle propio de cada tema plano, sobre las reglas comunes."""
    def vgrad(top: str, bottom: str) -> str:
        return f"qlineargradient(x1:0, y1:0, x2:0, y2:1, stop:0 {top}, stop:1 {bottom})"

    headers = (f"QHeaderView::section {{ color: {t['header_text']}; }}\n"
               if t["header_text"] != t["muted"] else "")
    flavors = {
        # Atardecer: barras con degradado violeta y encabezados con línea magenta.
        "synthwave": f"""
QMenuBar {{ background: {t['bar_top']}; }}
QToolBar#toolbar {{
    background: {vgrad(t['bar_top'], t['bg'])}; border-bottom: 2px solid {t['accent']};
}}
QHeaderView::section {{
    background: {vgrad(t['bar_top'], t['sidebar'])}; border-bottom: 2px solid {t['sunset']};
}}
""",
        # Neón: cian en encabezados y campo con foco iluminado.
        "neon": f"""
QHeaderView::section {{ border-bottom: 1px solid {t['accent']}; }}
QLineEdit:focus, QPlainTextEdit:focus, QComboBox:focus {{ background: {t['selection']}; }}
""",
        # Industrial: franjas gruesas de óxido.
        "rust": f"""
QToolBar#toolbar {{ border-bottom: 3px solid {t['accent']}; }}
QHeaderView::section {{ border-bottom: 2px solid {t['accent']}; }}
QFrame#server_card {{ border-top: 3px solid {t['accent']}; }}
""",
        # Madera: barra con degradado cálido y ficha con canto de acento.
        "brown": f"""
QMenuBar {{ background: {t['bar_top']}; }}
QToolBar#toolbar {{ background: {vgrad(t['bar_top'], t['bg'])}; border-bottom: 1px solid {t['border']}; }}
QFrame#server_card {{ border-left: 4px solid {t['accent']}; }}
""",
        # Militar: marca de acento junto al elemento seleccionado.
        "green": f"""
QTreeWidget#game_tree::item:selected, QListWidget::item:selected {{
    border-left: 3px solid {t['accent']};
}}
QHeaderView::section {{ border-bottom: 1px solid {t['accent']}; }}
""",
    }
    return headers + flavors.get(theme_id, "")


def _outline_rules(t: dict[str, str]) -> str:
    """Contorno de neón en la fila seleccionada de las tablas (arriba y abajo de cada celda)."""
    o = t["selection_outline"]
    return f"""
QTableView::item:selected {{
    background: {t['selection']}; color: {t['selection_text']};
    border-top: 1px solid {o}; border-bottom: 1px solid {o};
}}
"""


def _indicator_rules(t: dict[str, str], style: str) -> str:
    """Casillas visibles en cualquier fondo: caja con borde y, marcada, un cuadro relleno."""
    if style == "win98":
        return _win98_indicator_rules(t)
    if style == "luna":
        return _luna_indicator_rules(t)
    if style == "vgui":
        return _vgui_indicator_rules(t)
    if style == "terminal":
        return _terminal_indicator_rules(t)
    box = f"border: 1px solid {t['check_border']}; background: {t['surface']};"
    fill = t["accent"]
    radius = 2 if style == "flat" else 0
    sel = ("QCheckBox::indicator, QTreeView::indicator, QTableView::indicator, "
           "QListView::indicator")
    checked = sel.replace("::indicator", "::indicator:checked")
    partial = sel.replace("::indicator", "::indicator:indeterminate")
    disabled = sel.replace("::indicator", "::indicator:disabled")
    return f"""
{sel} {{
    width: 10px; height: 10px; padding: 2px; {box} border-radius: {radius}px;
}}
{checked}, {partial} {{ background: {fill}; background-clip: content; }}
{disabled} {{ border-color: {t['muted']}; }}
QMenu::indicator {{ width: 10px; height: 10px; padding: 2px; border: 1px solid {t['muted']}; }}
QMenu::indicator:checked {{ background: {fill}; background-clip: content; }}
QMenu::indicator:exclusive {{ border-radius: 7px; }}
"""


def _raised(t: dict[str, str]) -> str:
    return (f"border: 2px outset; border-color: {t['bevel_light']} {t['bevel_dark']} "
            f"{t['bevel_dark']} {t['bevel_light']};")


def _sunken(t: dict[str, str]) -> str:
    return (f"border: 2px inset; border-color: {t['bevel_dark']} {t['bevel_light']} "
            f"{t['bevel_light']} {t['bevel_dark']};")


def _sunken_on_surface(t: dict[str, str]) -> str:
    """Hundido para controles sobre fondo blanco: abajo/derecha con la luz interior.

    Con `solid`: `inset` aclara los bordes de abajo/derecha y la luz quedaría blanca.
    """
    return (f"border: 2px solid; border-color: {t['bevel_dark']} {t['bevel_inner']} "
            f"{t['bevel_inner']} {t['bevel_dark']};")


def _win98_img(name: str) -> str:
    return f'url("{(WIN98_DIR / f"{name}.svg").as_posix()}")'


def _luna_img(name: str) -> str:
    return f'url("{(LUNA_DIR / f"{name}.svg").as_posix()}")'


def _thin(t: dict[str, str], *, sunken: bool) -> str:
    """Bisel de 1 px: barras planas de 98 (botones de herramientas, menú, estado)."""
    light, dark = t["bevel_light"], t["border"]
    if sunken:
        light, dark = dark, light
    return f"border: 1px solid; border-color: {light} {dark} {dark} {light};"


def _win98_indicator_rules(t: dict[str, str]) -> str:
    """Casillas de 98: caja blanca hundida con ✓ negro; parcial, ✓ gris sobre gris."""
    sel = ("QCheckBox::indicator, QTreeView::indicator, QTableView::indicator, "
           "QListView::indicator")
    checked = sel.replace("::indicator", "::indicator:checked")
    partial = sel.replace("::indicator", "::indicator:indeterminate")
    disabled = sel.replace("::indicator", "::indicator:disabled")
    return f"""
{sel} {{
    width: 9px; height: 9px; padding: 1px; {_sunken_on_surface(t)} background: {t['surface']};
    border-radius: 0px;
}}
{checked} {{ image: {_win98_img("check")}; }}
{partial} {{ image: {_win98_img("check_partial")}; background: {t['bg']}; }}
{disabled} {{ background: {t['bg']}; }}
QMenu::indicator {{ width: 13px; height: 13px; border: none; background: transparent; }}
QMenu::indicator:checked {{ image: {_win98_img("check")}; }}
QMenu::indicator:exclusive:checked {{ image: {_win98_img("dot")}; }}
"""


def _win98_rules(t: dict[str, str]) -> str:
    """Windows 98: biselados de 2 px, botones en relieve, esquinas rectas."""
    raised, sunken = _raised(t), _sunken(t)
    thin_up, thin_down = _thin(t, sunken=False), _thin(t, sunken=True)
    face = t["button_top"]
    sel, sel_text = t["selection"], t["selection_text"]
    return f"""
QTableView, QListWidget, QTreeWidget#game_tree, QLineEdit, QPlainTextEdit, QComboBox, QFrame#server_card {{
    {sunken} border-radius: 0px;
}}
QListWidget, QTreeWidget#game_tree {{ background: {t['surface']}; }}
QListWidget::item, QTreeWidget#game_tree::item {{
    border-radius: 0px; padding: 1px 3px; border: 1px solid transparent;
}}
QListWidget::item:focus, QTreeWidget#game_tree::item:focus {{ border: 1px dotted {t['text']}; }}
QTableView:!focus, QListWidget:!focus, QTreeWidget#game_tree:!focus {{
    selection-background-color: {face}; selection-color: {t['text']};
}}
QListWidget::item:selected:!focus, QTreeWidget#game_tree::item:selected:!focus {{
    background: {face}; color: {t['text']};
}}
QLineEdit, QPlainTextEdit, QComboBox {{ padding: 2px 4px; selection-background-color: {sel}; selection-color: {sel_text}; }}
QLineEdit:focus, QPlainTextEdit:focus, QComboBox:focus {{ border-color: {t['bevel_dark']} {t['bevel_light']} {t['bevel_light']} {t['bevel_dark']}; }}
QLineEdit:disabled, QPlainTextEdit:disabled, QComboBox:disabled {{ background: {t['bg']}; }}
QComboBox::drop-down {{
    subcontrol-origin: padding; subcontrol-position: top right; width: 16px;
    background: {face}; {raised}
}}
QComboBox::drop-down:on {{ {thin_down} }}
QComboBox::down-arrow {{ image: {_win98_img("arrow_down")}; width: 7px; height: 4px; }}
QComboBox QAbstractItemView {{ border: 1px solid {t['text']}; outline: none; }}
QHeaderView::section {{
    background: {face}; color: {t['text']}; {raised} border-radius: 0px; padding: 2px 6px;
    font-weight: normal;
}}
QHeaderView::section:pressed {{ {sunken} }}
QHeaderView::down-arrow {{ image: {_win98_img("arrow_down")}; width: 7px; height: 4px; padding-right: 4px; }}
QHeaderView::up-arrow {{ image: {_win98_img("arrow_up")}; width: 7px; height: 4px; padding-right: 4px; }}
QPushButton {{
    background: {face}; color: {t['text']}; {raised} border-radius: 0px; padding: 4px 12px;
    min-width: 60px;
}}
QPushButton#copy_address, QPushButton#favorite_toggle {{ min-width: 0px; }}
QPushButton:pressed, QPushButton:checked {{ {sunken} }}
QPushButton:disabled {{ color: {t['border']}; }}
QPushButton:default {{ border: 3px solid; border-color: {t['text']}; }}
QPushButton:focus {{ outline: 1px dotted {t['text']}; }}
QToolBar#toolbar {{ border-bottom: 1px solid {t['border']}; }}
QToolBar#toolbar::separator {{
    background: {t['border']}; border-right: 1px solid {t['bevel_light']}; width: 1px;
}}
QToolButton {{ background: {face}; {raised} border-radius: 0px; padding: 3px 8px; }}
QToolButton:pressed, QToolButton:open {{ background: {face}; {sunken} }}
QToolBar#toolbar QToolButton {{
    background: {face}; border: 1px solid transparent; padding: 3px;
}}
QToolBar#toolbar QToolButton:hover {{ {thin_up} }}
QToolBar#toolbar QToolButton:pressed, QToolBar#toolbar QToolButton:checked,
QToolBar#toolbar QToolButton:open {{ {thin_down} }}
QToolBar#toolbar QToolButton:checked:!hover {{ background: {t['bevel_inner']}; }}
QMenuBar {{ border-bottom: 1px solid {t['bevel_light']}; }}
QMenuBar::item {{ padding: 2px 7px; border: 1px solid transparent; background: transparent; }}
QMenuBar::item:selected {{ background: {face}; color: {t['text']}; {thin_up} }}
QMenuBar::item:pressed {{ background: {face}; color: {t['text']}; {thin_down} }}
QMenu {{ background: {face}; {raised} padding: 1px; }}
QMenu::item {{ padding: 3px 20px 3px 22px; }}
QMenu::item:selected {{ background: {sel}; color: {sel_text}; }}
QMenu::item:disabled {{ color: {t['border']}; }}
QMenu::right-arrow {{ image: {_win98_img("arrow_right")}; width: 4px; height: 7px; padding-right: 6px; }}
QMenu::right-arrow:selected {{ image: {_win98_img("arrow_right_white")}; }}
QMenu::separator {{
    height: 0px; border-top: 1px solid {t['border']}; border-bottom: 1px solid {t['bevel_light']};
    margin: 3px 1px;
}}
QToolTip {{
    background: {t['tooltip']}; color: {t['text']}; border: 1px solid {t['text']}; padding: 1px 2px;
}}
QScrollBar:vertical {{ background: {_win98_img("checker")}; width: 16px; margin: 16px 0px; border: none; }}
QScrollBar:horizontal {{ background: {_win98_img("checker")}; height: 16px; margin: 0px 16px; border: none; }}
QScrollBar::handle {{ background: {face}; {raised} }}
QScrollBar::handle:vertical {{ min-height: 16px; }}
QScrollBar::handle:horizontal {{ min-width: 16px; }}
QScrollBar::add-line, QScrollBar::sub-line {{ background: {face}; {raised} subcontrol-origin: margin; }}
QScrollBar::add-line:pressed, QScrollBar::sub-line:pressed {{ {thin_down} }}
QScrollBar::add-line:vertical {{ height: 12px; subcontrol-position: bottom; }}
QScrollBar::sub-line:vertical {{ height: 12px; subcontrol-position: top; }}
QScrollBar::add-line:horizontal {{ width: 12px; subcontrol-position: right; }}
QScrollBar::sub-line:horizontal {{ width: 12px; subcontrol-position: left; }}
QScrollBar::up-arrow {{ image: {_win98_img("arrow_up")}; width: 7px; height: 4px; }}
QScrollBar::down-arrow {{ image: {_win98_img("arrow_down")}; width: 7px; height: 4px; }}
QScrollBar::left-arrow {{ image: {_win98_img("arrow_left")}; width: 4px; height: 7px; }}
QScrollBar::right-arrow {{ image: {_win98_img("arrow_right")}; width: 4px; height: 7px; }}
QScrollBar::add-page, QScrollBar::sub-page {{ background: none; }}
QAbstractScrollArea::corner {{ background: {face}; border: none; }}
QSplitter::handle {{ background: {face}; }}
QTabWidget::pane {{ {raised} background: {face}; top: -2px; }}
QTabBar::tab {{
    background: {face}; color: {t['text']}; {raised} border-bottom: none;
    padding: 4px 12px; margin-right: 1px;
}}
QTabBar::tab:selected {{ margin-bottom: -2px; padding-bottom: 6px; }}
QTabBar::tab:!selected {{ margin-top: 2px; }}
QProgressBar {{
    {thin_down} background: {face}; color: {t['text']}; text-align: center; max-height: 14px;
}}
QProgressBar::chunk {{ background: {sel}; margin: 2px 1px; width: 6px; }}
QFrame#server_card QLabel#map_image {{ {_sunken_on_surface(t)} border-radius: 0px; }}
QFrame#server_card QLabel#map_image[placeholder="false"] {{ border: none; }}
QStatusBar {{ border-top: 1px solid {t['bevel_light']}; }}
QStatusBar::item {{ border: none; }}
QStatusBar QLabel {{ {thin_down} padding: 0px 4px; }}
QStatusBar QLabel#scanning_label {{ border: none; }}
"""


def _tinted_x(color: str) -> str:
    """X de 7×7 en `color` (la marca de las casillas [X] de Terminal)."""
    return _tinted_svg(f"x_{color.lstrip('#')}",
                       '<svg xmlns="http://www.w3.org/2000/svg" width="7" height="7" '
                       'viewBox="0 0 7 7" shape-rendering="crispEdges"><path d="M0 0h2v1h1v1h1V1h1'
                       'V0h2v2H6v1H5v1h1v1h1v2H5V6H4V5H3v1H2v1H0V5h1V4h1V3H1V2H0z" '
                       f'fill="{color}"/></svg>')


def _tinted_dither(color: str) -> str:
    """Trama ▒ de 2×2 en `color` (pista del scroll de Terminal)."""
    return _tinted_svg(f"dither_{color.lstrip('#')}",
                       '<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2" '
                       'viewBox="0 0 2 2" shape-rendering="crispEdges">'
                       f'<rect width="1" height="1" fill="{color}"/>'
                       f'<rect x="1" y="1" width="1" height="1" fill="{color}"/></svg>')


def _terminal_indicator_rules(t: dict[str, str]) -> str:
    """Casillas [X] de Terminal: caja de línea fina y X en el color del fósforo."""
    sel = ("QCheckBox::indicator, QTreeView::indicator, QTableView::indicator, "
           "QListView::indicator")
    checked = sel.replace("::indicator", "::indicator:checked")
    partial = sel.replace("::indicator", "::indicator:indeterminate")
    disabled = sel.replace("::indicator", "::indicator:disabled")
    return f"""
{sel} {{
    width: 9px; height: 9px; padding: 2px; border: 1px solid {t['text']};
    background: {t['surface']}; border-radius: 0px;
}}
{checked} {{ image: {_tinted_x(t['text'])}; }}
{partial} {{ image: {_tinted_x(t['border'])}; }}
{disabled} {{ border-color: {t['border']}; }}
QMenu::indicator {{ width: 13px; height: 13px; border: none; background: transparent; }}
QMenu::indicator:checked {{ image: {_tinted_x(t['text'])}; }}
QMenu::indicator:checked:selected {{ image: {_tinted_x(t['selection_text'])}; }}
"""


def _terminal_rules(t: dict[str, str]) -> str:
    """Terminal de MS-DOS/Unix: monocromo, líneas finas, marcos dobles y video inverso."""
    ink, dim, paper = t["text"], t["border"], t["bg"]
    inverse = f"background: {ink}; color: {paper};"
    scan = (f"background-image: {_scanlines(t['scanline'])};"
            if t["scanline"] != t["bg"] else "")
    arrows = {d: _tinted_arrow(d, ink) for d in ("up", "down", "right")}
    return f"""
QTableView, QListWidget, QTreeWidget#game_tree, QLineEdit, QPlainTextEdit, QComboBox {{
    border: 1px solid {dim}; border-radius: 0px;
}}
QTableView, QListWidget, QTreeWidget#game_tree {{ background-color: {t['surface']}; {scan} }}
QListWidget::item, QTreeWidget#game_tree::item {{ border-radius: 0px; padding: 1px 4px; }}
QFrame#server_card {{ border: 3px double {ink}; border-radius: 0px; }}
QFrame#server_card QLabel#map_image {{ border: 1px dashed {dim}; border-radius: 0px; }}
QFrame#server_card QLabel#map_image[placeholder="false"] {{ border: none; }}
QLineEdit, QPlainTextEdit, QComboBox {{ background: {t['surface']}; padding: 2px 4px; }}
QLineEdit:focus, QPlainTextEdit:focus, QComboBox:focus {{ border-color: {ink}; }}
QComboBox::drop-down {{ border: none; width: 18px; }}
QComboBox::down-arrow {{ image: {arrows['down']}; width: 9px; height: 5px; }}
QComboBox QAbstractItemView {{ border: 1px solid {ink}; outline: none; }}
QHeaderView::section {{
    background: {paper}; color: {t['accent']}; border: none;
    border-bottom: 1px dashed {dim}; padding: 3px 6px; font-weight: normal;
}}
QHeaderView::down-arrow {{ image: {arrows['down']}; width: 9px; height: 5px; padding-right: 4px; }}
QHeaderView::up-arrow {{ image: {arrows['up']}; width: 9px; height: 5px; padding-right: 4px; }}
QPushButton, QToolButton#presets_button {{
    background: {paper}; color: {ink}; border: 1px solid {dim}; border-radius: 0px;
    padding: 3px 12px;
}}
QPushButton:hover, QPushButton:focus, QPushButton:pressed, QPushButton:checked,
QToolButton#presets_button:hover, QToolButton#presets_button:pressed {{
    {inverse} border-color: {ink};
}}
QPushButton:default {{ border: 3px double {ink}; padding: 1px 10px; }}
QPushButton:disabled {{ color: {dim}; background: {paper}; border-color: {dim}; }}
QToolBar#toolbar {{ background: {paper}; border-bottom: 1px solid {dim}; }}
QToolBar#toolbar::separator {{ background: {dim}; width: 1px; margin: 4px 6px; }}
QToolBar#toolbar QToolButton {{ background: transparent; border: 1px solid transparent; border-radius: 0px; }}
QToolBar#toolbar QToolButton:hover, QToolBar#toolbar QToolButton:open {{ border-color: {ink}; }}
QToolBar#toolbar QToolButton:pressed {{ border-color: {ink}; background: {t['surface']}; }}
QMenuBar {{ background: {paper}; color: {ink}; border-bottom: 1px solid {dim}; }}
QMenuBar::item {{ padding: 2px 8px; background: transparent; }}
QMenuBar::item:selected, QMenuBar::item:pressed {{ {inverse} }}
QMenu {{ background: {paper}; color: {ink}; border: 3px double {ink}; padding: 1px; }}
QMenu::item {{ padding: 3px 20px 3px 22px; }}
QMenu::item:selected {{ {inverse} }}
QMenu::item:disabled {{ color: {dim}; }}
QMenu::separator {{ height: 0px; border-top: 1px dashed {dim}; margin: 3px 4px; }}
QMenu::right-arrow {{ image: {arrows['right']}; width: 5px; height: 9px; padding-right: 6px; }}
QMenu::right-arrow:selected {{ image: {_tinted_arrow("right", paper)}; }}
QToolTip {{ {inverse} border: none; padding: 2px 6px; }}
QScrollBar:vertical {{ background: {_tinted_dither(dim)}; width: 14px; margin: 14px 0px; border: none; }}
QScrollBar:horizontal {{ background: {_tinted_dither(dim)}; height: 14px; margin: 0px 14px; border: none; }}
QScrollBar::handle {{ background: {ink}; border: none; border-radius: 0px; }}
QScrollBar::handle:vertical {{ min-height: 16px; }}
QScrollBar::handle:horizontal {{ min-width: 16px; }}
QScrollBar::add-line, QScrollBar::sub-line {{ background: {paper}; border: none; subcontrol-origin: margin; }}
QScrollBar::add-line:vertical {{ height: 14px; subcontrol-position: bottom; }}
QScrollBar::sub-line:vertical {{ height: 14px; subcontrol-position: top; }}
QScrollBar::add-line:horizontal {{ width: 14px; subcontrol-position: right; }}
QScrollBar::sub-line:horizontal {{ width: 14px; subcontrol-position: left; }}
QScrollBar::up-arrow {{ image: {arrows['up']}; width: 9px; height: 5px; }}
QScrollBar::down-arrow {{ image: {arrows['down']}; width: 9px; height: 5px; }}
QScrollBar::add-page, QScrollBar::sub-page {{ background: none; }}
QAbstractScrollArea::corner {{ background: {paper}; border: none; }}
QSplitter::handle {{ background: {paper}; }}
QTabWidget::pane {{ border: 1px solid {ink}; background: {paper}; top: -1px; }}
QTabBar::tab {{
    background: {paper}; color: {dim}; border: 1px solid {dim}; border-bottom: none;
    padding: 3px 12px; margin-right: 2px; border-radius: 0px;
}}
QTabBar::tab:selected {{ color: {ink}; border-color: {ink}; }}
QProgressBar {{
    border: 1px solid {dim}; background: {paper}; color: {ink}; text-align: center;
    max-height: 14px; border-radius: 0px;
}}
QProgressBar::chunk {{ background: {ink}; margin: 2px 1px; width: 6px; }}
QStatusBar {{ {inverse} }}
QStatusBar QLabel {{ background: transparent; color: {paper}; border: none; padding: 0px 6px; }}
QStatusBar QProgressBar {{ border-color: {paper}; background: {ink}; }}
QStatusBar QProgressBar::chunk {{ background: {paper}; }}
QStatusBar QSizeGrip {{ background: transparent; }}
"""


def _tinted_check(color: str) -> str:
    """✓ de 7×7 en `color`."""
    return _tinted_svg(f"check_{color.lstrip('#')}",
                       '<svg xmlns="http://www.w3.org/2000/svg" width="7" height="7" '
                       'viewBox="0 0 7 7" shape-rendering="crispEdges"><path d="M6 0h1v3H6v1H5v1'
                       'H4v1H3v1H2V6H1V5H0V2h1v1h1v1h1V3h1V2h1V1h1z" '
                       f'fill="{color}"/></svg>')


def _vgui_bevel(t: dict[str, str], *, sunken: bool) -> str:
    """Bisel de 1 px de VGUI: claro arriba/izquierda y oscuro abajo/derecha (al revés hundido)."""
    light, dark = t["bevel_light"], t["bevel_dark"]
    if sunken:
        light, dark = dark, light
    return f"border: 1px solid; border-color: {light} {dark} {dark} {light};"


def _vgui_indicator_rules(t: dict[str, str]) -> str:
    """Casillas de VGUI: caja hundida oscura con ✓ claro."""
    sel = ("QCheckBox::indicator, QTreeView::indicator, QTableView::indicator, "
           "QListView::indicator")
    checked = sel.replace("::indicator", "::indicator:checked")
    partial = sel.replace("::indicator", "::indicator:indeterminate")
    disabled = sel.replace("::indicator", "::indicator:disabled")
    return f"""
{sel} {{
    width: 9px; height: 9px; padding: 2px; {_vgui_bevel(t, sunken=True)}
    background: {t['surface']}; border-radius: 0px;
}}
{checked} {{ image: {_tinted_check(t['text'])}; }}
{partial} {{ image: {_tinted_check(t['muted'])}; }}
{disabled} {{ background: {t['bg']}; }}
QMenu::indicator {{ width: 13px; height: 13px; border: none; background: transparent; }}
QMenu::indicator:checked {{ image: {_tinted_check(t['text'])}; }}
"""


def _vgui_rules(t: dict[str, str]) -> str:
    """Steam de la era GoldSrc: oliva, biselados de 1 px, selección dorada, Tahoma."""
    raised, sunken = _vgui_bevel(t, sunken=False), _vgui_bevel(t, sunken=True)
    face, well = t["bg"], t["surface"]
    sel, sel_text = t["selection"], t["selection_text"]
    arrows = {d: _tinted_arrow(d, t["text"]) for d in ("up", "down")}
    return f"""
QTableView, QListWidget, QTreeWidget#game_tree, QLineEdit, QPlainTextEdit, QComboBox, QFrame#server_card {{
    {sunken} border-radius: 0px;
}}
QListWidget, QTreeWidget#game_tree {{ background: {well}; }}
QListWidget::item, QTreeWidget#game_tree::item {{ border-radius: 0px; padding: 2px 4px; }}
QLineEdit, QPlainTextEdit, QComboBox {{ background: {well}; padding: 2px 4px; }}
QLineEdit:focus, QPlainTextEdit:focus, QComboBox:focus {{ border-color: {t['bevel_dark']} {t['accent']} {t['accent']} {t['bevel_dark']}; }}
QLineEdit:disabled, QPlainTextEdit:disabled, QComboBox:disabled {{ color: {t['muted']}; background: {face}; }}
QComboBox::drop-down {{
    subcontrol-origin: padding; subcontrol-position: top right; width: 16px;
    background: {face}; {raised}
}}
QComboBox::down-arrow {{ image: {arrows['down']}; width: 9px; height: 5px; }}
QComboBox QAbstractItemView {{
    background: {well}; border: 1px solid {t['bevel_light']}; outline: none;
}}
QHeaderView::section {{
    background: {face}; color: {t['text']}; {raised} border-radius: 0px; padding: 3px 6px;
    font-weight: normal;
}}
QHeaderView::section:pressed {{ {sunken} }}
QHeaderView::down-arrow {{ image: {arrows['down']}; width: 9px; height: 5px; padding-right: 4px; }}
QHeaderView::up-arrow {{ image: {arrows['up']}; width: 9px; height: 5px; padding-right: 4px; }}
QPushButton {{
    background: {face}; color: {t['text']}; {raised} border-radius: 0px; padding: 4px 12px;
    min-width: 60px;
}}
QPushButton#copy_address, QPushButton#favorite_toggle {{ min-width: 0px; }}
QPushButton:hover {{ color: {t['accent']}; }}
QPushButton:pressed, QPushButton:checked {{ {sunken} }}
QPushButton:disabled {{ color: {t['muted']}; border-color: {t['bevel_dark']}; }}
QPushButton:default {{ border-color: {t['accent']} {t['bevel_dark']} {t['bevel_dark']} {t['accent']}; }}
QToolButton {{ background: {face}; color: {t['text']}; {raised} border-radius: 0px; padding: 3px 8px; }}
QToolButton:hover {{ color: {t['accent']}; }}
QToolButton:pressed, QToolButton:open {{ {sunken} }}
QToolBar#toolbar {{ background: {face}; border-bottom: 1px solid {t['bevel_dark']}; }}
QToolBar#toolbar::separator {{
    background: {t['bevel_dark']}; border-right: 1px solid {t['bevel_light']}; width: 1px;
}}
QToolBar#toolbar QToolButton {{ background: transparent; border: 1px solid transparent; padding: 3px; }}
QToolBar#toolbar QToolButton:hover {{ {raised} }}
QToolBar#toolbar QToolButton:pressed, QToolBar#toolbar QToolButton:checked,
QToolBar#toolbar QToolButton:open {{ {sunken} }}
QMenuBar {{ background: {face}; border-bottom: 1px solid {t['bevel_dark']}; }}
QMenuBar::item {{ padding: 2px 7px; background: transparent; }}
QMenuBar::item:selected, QMenuBar::item:pressed {{ background: {sel}; color: {sel_text}; }}
QMenu {{ background: {face}; {raised} padding: 1px; }}
QMenu::item {{ padding: 3px 20px 3px 22px; }}
QMenu::item:selected {{ background: {sel}; color: {sel_text}; }}
QMenu::item:disabled {{ color: {t['muted']}; }}
QMenu::separator {{
    height: 0px; border-top: 1px solid {t['bevel_dark']}; border-bottom: 1px solid {t['bevel_light']};
    margin: 3px 1px;
}}
QMenu::right-arrow {{ image: {_tinted_arrow("right", t['text'])}; width: 5px; height: 9px; padding-right: 6px; }}
QMenu::right-arrow:selected {{ image: {_tinted_arrow("right", t['selection_text'])}; }}
QToolTip {{
    background: {well}; color: {t['text']}; border: 1px solid {t['bevel_light']}; padding: 2px 4px;
}}
QScrollBar:vertical {{ background: {well}; width: 16px; margin: 16px 0px; border: none; }}
QScrollBar:horizontal {{ background: {well}; height: 16px; margin: 0px 16px; border: none; }}
QScrollBar::handle {{ background: {face}; {raised} }}
QScrollBar::handle:vertical {{ min-height: 20px; }}
QScrollBar::handle:horizontal {{ min-width: 20px; }}
QScrollBar::add-line, QScrollBar::sub-line {{ background: {face}; {raised} subcontrol-origin: margin; }}
QScrollBar::add-line:pressed, QScrollBar::sub-line:pressed {{ {sunken} }}
QScrollBar::add-line:vertical {{ height: 14px; subcontrol-position: bottom; }}
QScrollBar::sub-line:vertical {{ height: 14px; subcontrol-position: top; }}
QScrollBar::add-line:horizontal {{ width: 14px; subcontrol-position: right; }}
QScrollBar::sub-line:horizontal {{ width: 14px; subcontrol-position: left; }}
QScrollBar::up-arrow {{ image: {arrows['up']}; width: 9px; height: 5px; }}
QScrollBar::down-arrow {{ image: {arrows['down']}; width: 9px; height: 5px; }}
QScrollBar::add-page, QScrollBar::sub-page {{ background: none; }}
QAbstractScrollArea::corner {{ background: {face}; border: none; }}
QSplitter::handle {{ background: {face}; }}
QTabWidget::pane {{ {raised} background: {face}; top: -1px; }}
QTabBar::tab {{
    background: {face}; color: {t['muted']}; {raised} border-bottom: none;
    padding: 4px 12px; margin-right: 1px;
}}
QTabBar::tab:selected {{ color: {t['text']}; margin-bottom: -1px; padding-bottom: 5px; }}
QTabBar::tab:!selected {{ margin-top: 2px; }}
QProgressBar {{
    {sunken} background: {well}; color: {t['text']}; text-align: center; max-height: 14px;
}}
QProgressBar::chunk {{ background: {sel}; margin: 2px 1px; width: 6px; }}
QFrame#server_card QLabel#map_image {{ {sunken} border-radius: 0px; }}
QFrame#server_card QLabel#map_image[placeholder="false"] {{ border: none; }}
QStatusBar {{ border-top: 1px solid {t['bevel_light']}; }}
QStatusBar::item {{ border: none; }}
QStatusBar QLabel {{ {sunken} padding: 0px 4px; }}
QStatusBar QLabel#scanning_label {{ border: none; }}
"""


def _luna_indicator_rules(t: dict[str, str]) -> str:
    """Casillas de XP: caja con degradé y borde azul, ✓ verde; en menús, solo la marca."""
    sel = ("QCheckBox::indicator, QTreeView::indicator, QTableView::indicator, "
           "QListView::indicator")
    checked = sel.replace("::indicator", "::indicator:checked")
    partial = sel.replace("::indicator", "::indicator:indeterminate")
    hover = sel.replace("::indicator", "::indicator:hover")
    disabled = sel.replace("::indicator", "::indicator:disabled")
    box = (f"qlineargradient(x1:0, y1:0, x2:1, y2:1, stop:0 {t['check_top']}, "
           f"stop:1 {t['surface']})")
    glow = (f"qlineargradient(x1:0, y1:0, x2:1, y2:1, stop:0 {t['hover_light']}, "
            f"stop:0.5 {t['surface']}, stop:1 {t['hover']})")
    return f"""
{sel} {{
    width: 9px; height: 9px; padding: 2px; border: 1px solid {t['check_border']};
    background: {box}; border-radius: 0px;
}}
{hover} {{ background: {glow}; }}
{checked} {{ image: {_luna_img("check")}; }}
{partial} {{ image: {_luna_img("check_partial")}; }}
{disabled} {{ border-color: {t['disabled_border']}; background: {t['disabled_bg']}; }}
QMenu::indicator {{ width: 13px; height: 13px; border: none; background: transparent; }}
QMenu::indicator:checked {{ image: {_win98_img("check")}; }}
QMenu::indicator:exclusive:checked {{ image: {_win98_img("dot")}; }}
"""


def _luna_rules(t: dict[str, str]) -> str:
    """Windows XP (Luna): botones con borde azul y brillo naranja, scroll celeste, ✓ verde."""
    fb, bb = t["field_border"], t["button_border"]
    sel, sel_text = t["selection"], t["selection_text"]

    def vgrad(top: str, bottom: str, mid: str | None = None) -> str:
        middle = f"stop:0.85 {mid}, " if mid else ""
        return (f"qlineargradient(x1:0, y1:0, x2:0, y2:1, stop:0 {top}, {middle}"
                f"stop:1 {bottom})")

    def hgrad(left: str, right: str) -> str:
        return f"qlineargradient(x1:0, y1:0, x2:1, y2:0, stop:0 {left}, stop:1 {right})"

    button = vgrad(t["surface"], t["button_bottom"], t["button_top"])
    hover = (f"qlineargradient(x1:0, y1:0, x2:0, y2:1, stop:0 {t['hover_light']}, "
             f"stop:0.15 {t['surface']}, stop:0.8 {t['button_top']}, stop:1 {t['hover']})")
    default = (f"qlineargradient(x1:0, y1:0, x2:0, y2:1, stop:0 {t['default_top']}, "
               f"stop:0.15 {t['surface']}, stop:0.8 {t['button_top']}, "
               f"stop:1 {t['default_bottom']})")
    pressed = vgrad(t["pressed_top"], t["pressed_bottom"])
    scroll_v = hgrad(t["scroll_top"], t["scroll_bottom"])
    scroll_h = vgrad(t["scroll_top"], t["scroll_bottom"])
    scroll_hover_v = hgrad(t["surface"], t["scroll_top"])
    scroll_hover_h = vgrad(t["surface"], t["scroll_top"])
    tab = vgrad(t["surface"], t["bg"])
    bar = vgrad(t["button_top"], t["bg"])
    return f"""
QTableView, QListWidget, QTreeWidget#game_tree, QLineEdit, QPlainTextEdit, QComboBox {{
    border: 1px solid {fb}; border-radius: 0px;
}}
QListWidget, QTreeWidget#game_tree {{ background: {t['surface']}; }}
QListWidget::item, QTreeWidget#game_tree::item {{
    border-radius: 0px; padding: 1px 3px; border: 1px solid transparent;
}}
QListWidget::item:focus, QTreeWidget#game_tree::item:focus {{ border: 1px dotted {t['text']}; }}
QTableView:!focus, QListWidget:!focus, QTreeWidget#game_tree:!focus {{
    selection-background-color: {t['bg']}; selection-color: {t['text']};
}}
QListWidget::item:selected:!focus, QTreeWidget#game_tree::item:selected:!focus {{
    background: {t['bg']}; color: {t['text']};
}}
QLineEdit, QPlainTextEdit, QComboBox {{ padding: 2px 4px; selection-background-color: {sel}; selection-color: {sel_text}; }}
QLineEdit:focus, QPlainTextEdit:focus, QComboBox:focus {{ border-color: {fb}; }}
QLineEdit:disabled, QPlainTextEdit:disabled, QComboBox:disabled {{
    background: {t['disabled_bg']}; color: {t['disabled_text']}; border-color: {t['disabled_border']};
}}
QComboBox::drop-down {{
    subcontrol-origin: padding; subcontrol-position: top right; width: 15px; margin: 1px;
    background: {scroll_h}; border: 1px solid {t['scroll_border']}; border-radius: 2px;
}}
QComboBox::drop-down:hover {{ background: {scroll_hover_h}; }}
QComboBox::drop-down:on {{ background: {t['scroll_bottom']}; }}
QComboBox::down-arrow {{ image: {_luna_img("arrow_down")}; width: 7px; height: 4px; }}
QComboBox QAbstractItemView {{ border: 1px solid {t['text']}; outline: none; }}
QHeaderView::section {{
    background: {t['header_bg']}; color: {t['text']}; border: none;
    border-right: 1px solid {t['header_line']}; border-bottom: 3px solid {t['header_line']};
    padding: 3px 6px; font-weight: normal;
}}
QHeaderView::section:hover {{ background: {t['pane']}; border-bottom-color: {t['hover']}; }}
QHeaderView::section:pressed {{ background: {t['header_line']}; }}
QHeaderView::down-arrow {{ image: {_luna_img("sort_down")}; width: 7px; height: 4px; padding-right: 4px; }}
QHeaderView::up-arrow {{ image: {_luna_img("sort_up")}; width: 7px; height: 4px; padding-right: 4px; }}
QPushButton {{
    background: {button}; color: {t['text']};
    border: 1px solid {bb}; border-radius: 3px; padding: 4px 12px; min-width: 60px;
}}
QPushButton#copy_address, QPushButton#favorite_toggle {{ min-width: 0px; }}
QPushButton:default {{ background: {default}; }}
QPushButton:hover {{ background: {hover}; }}
QPushButton:pressed, QPushButton:checked {{ background: {pressed}; }}
QPushButton:disabled {{
    background: {t['disabled_bg']}; color: {t['disabled_text']}; border-color: {t['disabled_border']};
}}
QPushButton:focus {{ outline: none; }}
QToolButton {{
    background: {button}; border: 1px solid {bb}; border-radius: 3px; padding: 3px 8px;
}}
QToolButton:hover {{ background: {hover}; }}
QToolButton:pressed, QToolButton:open {{ background: {pressed}; }}
QToolBar#toolbar {{ background: {bar}; border-bottom: 1px solid {t['border']}; }}
QToolBar#toolbar QToolButton {{
    background: transparent; border: 1px solid transparent; border-radius: 3px; padding: 3px;
}}
QToolBar#toolbar QToolButton:hover {{ background: {button}; border-color: {t['border']}; }}
QToolBar#toolbar QToolButton:pressed, QToolBar#toolbar QToolButton:open,
QToolBar#toolbar QToolButton:checked {{ background: {pressed}; border-color: {t['border']}; }}
QMenuBar {{ background: {t['bg']}; }}
QMenuBar::item {{ padding: 2px 7px; background: transparent; }}
QMenuBar::item:selected, QMenuBar::item:pressed {{ background: {sel}; color: {sel_text}; }}
QMenu {{ background: {t['surface']}; border: 1px solid {t['disabled_text']}; padding: 2px; }}
QMenu::item {{ padding: 3px 20px 3px 22px; }}
QMenu::item:selected {{ background: {sel}; color: {sel_text}; }}
QMenu::item:disabled {{ color: {t['disabled_text']}; }}
QMenu::separator {{ height: 1px; background: {t['disabled_text']}; margin: 3px 1px; }}
QMenu::right-arrow {{ image: {_win98_img("arrow_right")}; width: 4px; height: 7px; padding-right: 6px; }}
QMenu::right-arrow:selected {{ image: {_win98_img("arrow_right_white")}; }}
QToolTip {{
    background: {t['tooltip']}; color: {t['text']}; border: 1px solid {t['text']}; padding: 1px 2px;
}}
QScrollBar:vertical {{ background: {hgrad(t['scroll_track'], t['surface'])}; width: 17px; margin: 17px 0px; border: none; }}
QScrollBar:horizontal {{ background: {vgrad(t['scroll_track'], t['surface'])}; height: 17px; margin: 0px 17px; border: none; }}
QScrollBar::handle {{ border: 1px solid {t['scroll_border']}; border-radius: 3px; margin: 1px; }}
QScrollBar::handle:vertical {{ background: {scroll_v}; min-height: 20px; image: {_luna_img("grip_v")}; }}
QScrollBar::handle:horizontal {{ background: {scroll_h}; min-width: 20px; image: {_luna_img("grip_h")}; }}
QScrollBar::handle:vertical:hover {{ background: {scroll_hover_v}; }}
QScrollBar::handle:horizontal:hover {{ background: {scroll_hover_h}; }}
QScrollBar::add-line, QScrollBar::sub-line {{
    border: 1px solid {t['scroll_border']}; border-radius: 3px; subcontrol-origin: margin; margin: 1px;
}}
QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {{ background: {scroll_v}; height: 15px; }}
QScrollBar::add-line:horizontal, QScrollBar::sub-line:horizontal {{ background: {scroll_h}; width: 15px; }}
QScrollBar::add-line:vertical {{ subcontrol-position: bottom; }}
QScrollBar::sub-line:vertical {{ subcontrol-position: top; }}
QScrollBar::add-line:horizontal {{ subcontrol-position: right; }}
QScrollBar::sub-line:horizontal {{ subcontrol-position: left; }}
QScrollBar::add-line:pressed, QScrollBar::sub-line:pressed {{ background: {t['scroll_bottom']}; }}
QScrollBar::up-arrow {{ image: {_luna_img("arrow_up")}; width: 7px; height: 4px; }}
QScrollBar::down-arrow {{ image: {_luna_img("arrow_down")}; width: 7px; height: 4px; }}
QScrollBar::left-arrow {{ image: {_luna_img("arrow_left")}; width: 4px; height: 7px; }}
QScrollBar::right-arrow {{ image: {_luna_img("arrow_right")}; width: 4px; height: 7px; }}
QScrollBar::add-page, QScrollBar::sub-page {{ background: none; }}
QAbstractScrollArea::corner {{ background: {t['bg']}; border: none; }}
QFrame#server_card {{ border: 1px solid {fb}; border-radius: 3px; }}
QTabWidget::pane {{ border: 1px solid {t['tab_border']}; background: {t['pane']}; top: -1px; }}
QTabWidget::pane QWidget {{ background: {t['pane']}; }}
QTabBar::tab {{
    background: {tab}; color: {t['text']}; border: 1px solid {t['tab_border']};
    border-bottom: none; border-top-left-radius: 3px; border-top-right-radius: 3px;
    padding: 3px 10px; margin-right: 1px;
}}
QTabBar::tab:hover {{ border-top: 2px solid {t['hover']}; }}
QTabBar::tab:selected {{
    background: {t['pane']}; border-top: 3px solid {t['hover']}; margin-bottom: -1px;
    padding-bottom: 4px;
}}
QTabBar::tab:!selected {{ margin-top: 2px; }}
QProgressBar {{
    border: 1px solid {t['progress_border']}; border-radius: 3px; background: {t['surface']};
    color: {t['text']}; text-align: center; max-height: 14px;
}}
QProgressBar::chunk {{
    background: {vgrad(t['progress_light'], t['progress'])}; margin: 2px 1px; width: 6px;
}}
QStatusBar::item {{ border: none; }}
QStatusBar QLabel {{ border-left: 1px solid {t['disabled_text']}; padding: 0px 6px; }}
QStatusBar QLabel#scanning_label {{ border: none; }}
"""


def _font_rules(theme_id: str) -> str:
    family = FONTS[theme_id]
    if family is None:
        return ""
    if STYLE[theme_id] in ("win98", "luna", "vgui"):  # 8 pt de época; la ficha, algo mayor
        return (f"\nQWidget {{ font-family: {family}; font-size: 8pt; }}\n"
                f"QLabel#card_name {{ font-size: 9pt; }}\n")
    return f"\nQWidget {{ font-family: {family}; }}\n"
