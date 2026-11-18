//! Nombres con códigos de color de Quake 3 / CoD (`^0`-`^9`) partidos en tramos.

use iced::Color;
use jase_core::protocols::strip_control;

use crate::theme::{Palette, legible};

/// Parte `raw` en tramos `(texto, color)`; `None` = color de texto normal. El texto unido
/// es `strip_colors(raw)`: se reconocen los mismos códigos que `COLOR_RE` de `quake3.py`.
pub fn color_runs(raw: &str, p: &Palette) -> Vec<(String, Option<Color>)> {
    let chars: Vec<char> = strip_control(raw).chars().collect();
    let mut runs: Vec<(String, Option<Color>)> = Vec::new();
    let mut color = None;
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '^' {
            let next = chars.get(i + 1).copied();
            let digit2 = chars.get(i + 2).copied().filter(char::is_ascii_digit);
            // `^^N` y `^^NN`: el último carácter es el código, como `m.group()[-1]`.
            let code = if next == Some('^')
                && let Some(d) = digit2
            {
                i += if chars.get(i + 3) == Some(&d) { 4 } else { 3 };
                Some(d)
            } else if let Some(c) = next.filter(char::is_ascii_alphanumeric) {
                i += 2;
                Some(c)
            } else {
                None
            };
            if let Some(code) = code {
                if let Some(d) = code.to_digit(10) {
                    color = p.q3[d as usize];
                }
                continue;
            }
        }
        match runs.last_mut() {
            Some((text, c)) if *c == color => text.push(chars[i]),
            _ => runs.push((chars[i].to_string(), color)),
        }
        i += 1;
    }
    runs
}

/// Color del texto propio de una celda (tramos `^N`, ping). Sin color, el de texto (o el de
/// la selección); en una fila seleccionada, un color que no se lee sobre ella pasa al de la
/// selección (p. ej. verde sobre el azul marino de Clásico).
pub fn cell_text_color(color: Option<Color>, selected: bool, p: &Palette) -> Color {
    let default = if selected { p.selection_text } else { p.text };
    match color {
        None => default,
        Some(c) if selected => legible(c, p.selection, default),
        Some(c) => c,
    }
}
