"""Genera rust/jase/src/theme/palette.rs a partir de src/jase/ui/theme.py.

Uso: QT_QPA_PLATFORM=offscreen python scripts/gen_palette_rs.py [--stdout]

Mientras Python siga en el repo, la paleta de Rust no se edita a mano: se regenera con
este script (--stdout imprime en vez de escribir; lo usa el test de Rust para compararla).
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "src"))

from jase.ui.theme import Q3_COLORS, STYLE, TOKENS, VARIANT_IDS  # noqa: E402

OUT = ROOT / "rust" / "jase" / "src" / "theme" / "palette.rs"
STYLES = {"flat": "Flat", "win98": "Win98", "luna": "Luna", "vgui": "Vgui", "terminal": "Terminal"}


def color(value: str) -> str:
    assert len(value) == 7 and value[0] == "#", value
    return f"rgb(0x{value[1:]})"


def render() -> str:
    keys = sorted(TOKENS["dark"])
    q3_keys = [str(i) for i in range(10)]
    for tid in VARIANT_IDS:
        assert sorted(TOKENS[tid]) == keys, tid
        assert sorted(Q3_COLORS[tid]) == q3_keys, tid
    out = ["// Generado por scripts/gen_palette_rs.py desde src/jase/ui/theme.py (Python 1.3.0).",
           "// No se edita a mano: se regenera con el script y lo verifica tests/theme.rs.",
           "use iced::Color;", "", "use super::WidgetStyle;", "",
           "const fn rgb(v: u32) -> Color {",
           "    Color::from_rgb8((v >> 16) as u8, (v >> 8) as u8, v as u8)", "}", "",
           "/// Colores de un tema: un campo por token de `TOKENS` y `q3` con `^0`-`^9`"
           " (`None` = texto normal).",
           "#[derive(Debug, Clone, Copy, PartialEq)]", "pub struct Palette {"]
    out += [f"    pub {k}: Color," for k in keys]
    out += ["    pub q3: [Option<Color>; 10],", "}", "",
            f"pub static VARIANTS: [(&str, Palette, WidgetStyle); {len(VARIANT_IDS)}] = ["]
    for tid in VARIANT_IDS:
        out.append(f'    ("{tid}", Palette {{')
        out += [f"        {k}: {color(TOKENS[tid][k])}," for k in keys]
        q3 = ", ".join("None" if Q3_COLORS[tid][i] is None else f"Some({color(Q3_COLORS[tid][i])})"
                       for i in q3_keys)
        out += [f"        q3: [{q3}],", f"    }}, WidgetStyle::{STYLES[STYLE[tid]]}),"]
    out.append("];")
    badges = [k.removeprefix("badge_") for k in keys if k.startswith("badge_")]
    out += ["", "impl Palette {",
            "    /// Color de la insignia del juego (`badge_<id>`); `None` si el juego no tiene.",
            "    pub fn badge(&self, game_id: &str) -> Option<Color> {",
            "        match game_id {"]
    out += [f'            "{g}" => Some(self.badge_{g}),' for g in badges]
    out += ["            _ => None,", "        }", "    }", "}"]
    return "\n".join(out) + "\n"


def main() -> int:
    text = render()
    if "--stdout" in sys.argv:
        sys.stdout.buffer.write(text.encode("utf-8"))
    else:
        OUT.parent.mkdir(parents=True, exist_ok=True)
        OUT.write_bytes(text.encode("utf-8"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
