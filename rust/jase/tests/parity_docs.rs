//! La guía de uso atada a la tabla «Paridad de la app» de `port-rust.md`: cada punto de la guía
//! tiene su fila y ninguna fila queda «pendiente» sin dueño.

use std::path::{Path, PathBuf};

/// Puntos de la guía que no son de la app de escritorio, con su motivo.
const NOT_APP: &[(&str, &str)] = &[
    (
        "Instalar y ejecutar desde el código",
        "instrucciones de la app Python; se reescriben en la fase 7",
    ),
    ("Tests", "los tests de Python; se reescriben en la fase 7"),
];

/// Puntos de la guía cuya fila lleva otro nombre (ya normalizados): (punto, fila).
const ALIASES: &[(&str, &str)] = &[
    ("créditos", "acerca de y créditos"),
    ("acerca de", "acerca de y créditos"),
    (
        "juegos de steam y la clave de la api web de steam",
        "juegos de steam y la clave de la api web",
    ),
];

/// Quien se hace cargo de una fila «pendiente».
const OWNERS: &[&str] = &["del dueño", "fase 6", "fase 7"];

fn doc(parts: &[&str]) -> String {
    let mut path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    for part in parts {
        path = path.join(part);
    }
    // En Windows git puede convertir los saltos de línea al sacar el archivo.
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .replace("\r\n", "\n")
}

/// Texto de un enlace `[texto](destino)` sin el destino.
fn strip_links(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find("](").and_then(|mid| {
            let close = after[mid..].find(')')? + mid;
            Some((mid, close))
        }) {
            Some((mid, close)) => {
                out.push_str(&after[..mid]);
                rest = &after[close + 1..];
            }
            None => {
                out.push('[');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out.trim().to_string()
}

/// Títulos desde `## Descarga` (sin los contenedores) y viñetas `**Nombre**` de `## Uso`.
fn guide_points(guide: &str) -> Vec<String> {
    let mut points = Vec::new();
    let mut started = false;
    let mut in_use = false;
    for line in guide.lines() {
        if let Some(title) = line
            .strip_prefix("## ")
            .or_else(|| line.strip_prefix("### "))
        {
            let title = strip_links(title);
            started |= title == "Descarga";
            if line.starts_with("## ") {
                in_use = title == "Uso";
            }
            if started && title != "Uso" && title != "Más detalles" {
                points.push(title);
            }
        } else if in_use
            && let Some(rest) = line.strip_prefix("- **")
            && let Some(end) = rest.find("**")
        {
            points.push(strip_links(rest[..end].trim_end_matches(':')));
        }
    }
    points
}

/// Filas de la tabla entre `## Paridad de la app` y el siguiente `## `: (punto, estado).
fn parity_rows(roadmap: &str) -> Vec<(String, String)> {
    roadmap
        .lines()
        .skip_while(|line| *line != "## Paridad de la app")
        .skip(1)
        .take_while(|line| !line.starts_with("## "))
        .filter(|line| line.starts_with('|') && !line.starts_with("|---"))
        .skip(1) // encabezado
        .map(|line| {
            let fields: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
            let state = fields.get(2).copied().unwrap_or_default();
            (fields[0].to_string(), state.to_string())
        })
        .collect()
}

fn guide() -> String {
    doc(&["docs", "guia", "guia-usuario.md"])
}

fn roadmap() -> String {
    doc(&["docs", "hoja-de-ruta", "port-rust.md"])
}

#[test]
fn guide_points_are_found() {
    let points = guide_points(&guide());
    assert!(
        points.len() >= 30,
        "solo {} puntos: {points:?}",
        points.len()
    );
    for expected in [
        "Filtros rápidos",
        "Bandeja del sistema",
        "País de los servidores",
    ] {
        assert!(
            points.iter().any(|p| p == expected),
            "falta {expected}: {points:?}"
        );
    }
    assert!(!points.iter().any(|p| p == "Uso" || p == "Más detalles"));
    assert!(!points.iter().any(|p| p.contains('[') || p.ends_with(':')));
    let rows = parity_rows(&roadmap());
    assert!(rows.len() >= 30, "solo {} filas", rows.len());
    assert!(!rows.iter().any(|(point, _)| point == "Punto de la guía"));
}

/// Minúsculas y sin el paréntesis final: «Ventana (tamaño, …)» se nombra «ventana».
fn normalize(name: &str) -> String {
    let low = name.trim().to_lowercase();
    let base = match low.rfind(" (") {
        Some(i) if low.ends_with(')') => &low[..i],
        _ => low.as_str(),
    };
    base.trim().to_string()
}

/// Alguna fila se llama como el punto (o su alias); contener al otro no basta.
fn covers(rows: &[String], point: &str) -> bool {
    let point = normalize(point);
    let point = ALIASES
        .iter()
        .find(|(alias, _)| *alias == point)
        .map_or(point.as_str(), |(_, row)| row);
    rows.iter().any(|row| normalize(row) == point)
}

#[test]
fn every_guide_point_has_a_parity_row() {
    let rows: Vec<String> = parity_rows(&roadmap())
        .into_iter()
        .map(|(p, _)| p)
        .collect();
    for point in guide_points(&guide()) {
        if NOT_APP.iter().any(|(p, _)| *p == point) {
            continue;
        }
        assert!(
            covers(&rows, &point),
            "punto de la guía sin fila en Paridad de la app: {point}"
        );
    }
}

#[test]
fn a_row_containing_another_point_does_not_cover_it() {
    let rows = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<Vec<_>>();
    assert!(!covers(&rows(&["Notas de favoritos"]), "Favoritos"));
    assert!(!covers(&rows(&["Favoritos"]), "Notas de favoritos"));
    assert!(covers(&rows(&["Favoritos"]), "favoritos"));
    assert!(covers(&rows(&["Ventana (tamaño, divisiones)"]), "Ventana"));
    assert!(covers(&rows(&["Acerca de y créditos"]), "Créditos"));
    assert!(!covers(&rows(&["Acerca de y créditos"]), "Acerca"));
}

#[test]
fn no_pending_without_owner() {
    for (point, state) in parity_rows(&roadmap()) {
        let low = state.to_lowercase();
        assert!(
            !low.contains("pendiente") || OWNERS.iter().any(|o| low.contains(o)),
            "fila «pendiente» sin dueño (del dueño, fase 6 o fase 7): {point} | {state}"
        );
    }
}

#[test]
fn not_app_points_are_in_the_guide() {
    let points = guide_points(&guide());
    for (point, _) in NOT_APP {
        assert!(points.iter().any(|p| p == point), "NOT_APP sobra: {point}");
    }
}
