//! Verificador de la documentación (port de `check_docs.py` de 1.3.0): enlaces rotos, rutas
//! inexistentes, datos personales y etapas de la memoria.
//!
//! - Enlaces relativos y rutas del repo entre comillas invertidas: `README*.md`, `AGENTS.md` y
//!   `docs/` (salvo `docs/superpowers/`, notas locales). Solo se revisan las rutas cuyo primer
//!   segmento es una entrada de la raíz del repo (`rust/…`, `docs/…`).
//! - Datos personales (rutas de unidad, /home, /mnt, IPv4 fuera de los rangos de
//!   documentación, claves hex): solo `AGENTS.md` y `docs/`. Los README citan masters reales.
//! - `docs/ai/PROJECT_MEMORY.md`: etapas `## AAAA-MM-DD — tema` con 3 viñetas como mucho.
//! - En un repo git, «existe» = está versionado (`target/` o `dist/` locales no cuentan).

use std::path::Path;
use std::process::Command;

#[test]
fn real_repository_is_clean() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    assert_eq!(check(&root), Vec::<String>::new());
}

fn problems(files: &[(&str, &str)]) -> Vec<String> {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), files);
    check(dir.path())
}

fn write(root: &Path, files: &[(&str, &str)]) {
    for (name, text) in files {
        let path = name.split('/').fold(root.to_path_buf(), |p, s| p.join(s));
        std::fs::create_dir_all(path.parent().expect("padre")).expect("mkdir");
        std::fs::write(&path, text).expect("write");
    }
}

#[test]
fn broken_relative_link() {
    let found = problems(&[(
        "docs/README.md",
        "[x](no-existe.md) [y](https://e.org) [z](#a)",
    )]);
    assert_eq!(found, vec!["docs/README.md:1: enlace roto: no-existe.md"]);
}

#[test]
fn link_with_anchor_to_existing_file_is_fine() {
    let found = problems(&[("docs/a.md", "x"), ("docs/README.md", "[a](a.md#seccion)")]);
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn backtick_repo_path_must_exist() {
    let found = problems(&[
        ("rust/x.rs", ""),
        (
            "docs/README.md",
            "`rust/x.rs` `rust/falta.rs` `rust/x.rs:12` `rust/*.rs`",
        ),
    ]);
    assert_eq!(
        found,
        vec!["docs/README.md:1: ruta inexistente: rust/falta.rs"]
    );
}

#[test]
fn backticks_outside_repo_roots_are_ignored() {
    let found = problems(&[
        ("src/a.py", ""),
        (
            "docs/README.md",
            "`code/client/cl_main.cpp` `System\\UT.exe` `jase://x` `<ruta>/a.py` \
             `src/<familia>.py` `tests/fixtures/<juego>_*.bin`",
        ),
    ]);
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn personal_data_in_docs() {
    let text = "C:\\Users\\ana\\juego\n/home/ana/x\n/mnt/c/Users/ana\n8.8.4.4\n\
                clave 0123456789abcdef0123456789ABCDEF\n";
    let found = problems(&[("docs/x.md", text)]);
    assert_eq!(found.len(), 5, "{found:?}");
    assert_eq!(found[0], "docs/x.md:1: dato personal (ruta de unidad)");
    assert_eq!(
        found[3],
        "docs/x.md:4: dato personal (IPv4 8.8.4.4; usar 192.0.2.x, 198.51.100.x o 203.0.113.x)"
    );
    assert_eq!(found[4], "docs/x.md:5: dato personal (posible clave)");
}

#[test]
fn documentation_ranges_and_placeholders_are_allowed() {
    let text = "192.0.2.1 198.51.100.7 203.0.113.9 127.0.0.1 0.0.0.0 255.255.255.255\n\
                <CARPETA_DEL_JUEGO>\\System `%APPDATA%\\JimmysASE`\n";
    assert_eq!(problems(&[("docs/x.md", text)]), Vec::<String>::new());
}

#[test]
fn readme_is_not_checked_for_personal_data() {
    let found = problems(&[("README.md", "master 198.18.0.1 y 8.8.4.4")]);
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn local_plans_are_excluded() {
    let found = problems(&[("docs/superpowers/p.md", "[x](falta.md) /home/ana")]);
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn memory_stage_with_more_than_three_bullets() {
    let memory = "# Memoria\n\n## 2026-11-05 — bien\n- a\n- b\n- c\n\n\
                  ## 2026-11-04 — mal\n- a\n- b\n- c\n- d\n  - subnivel no cuenta\n";
    let found = problems(&[("docs/ai/PROJECT_MEMORY.md", memory)]);
    assert_eq!(
        found,
        vec!["docs/ai/PROJECT_MEMORY.md:8: etapa con 4 viñetas (máximo 3): 2026-11-04 — mal"]
    );
}

#[test]
fn memory_stage_heading_format() {
    let memory = "# Memoria\n\n## ayer — sin fecha\n- a\n";
    let found = problems(&[("docs/ai/PROJECT_MEMORY.md", memory)]);
    assert_eq!(
        found,
        vec!["docs/ai/PROJECT_MEMORY.md:3: etapa sin el formato «## AAAA-MM-DD — tema»"]
    );
}

#[test]
fn ip_at_end_of_sentence_is_flagged() {
    assert_eq!(problems(&[("docs/x.md", "Servidor 8.8.4.4.")]).len(), 1);
}

#[test]
fn titled_and_reference_links_are_checked() {
    let text = "[x](falta.md \"título\")\n\n[r]: otra-falta.md\n";
    let found = problems(&[("docs/README.md", text)]);
    assert_eq!(
        found,
        vec![
            "docs/README.md:1: enlace roto: falta.md",
            "docs/README.md:3: enlace roto: otra-falta.md",
        ]
    );
}

#[test]
fn forward_slash_drive_path_is_personal() {
    assert_eq!(problems(&[("docs/x.md", "C:/Users/ana/juego")]).len(), 1);
}

#[test]
fn star_bullets_count_in_memory() {
    let memory = "# M\n\n## 2026-11-04 — t\n* a\n* b\n- c\n* d\n";
    assert_eq!(problems(&[("docs/ai/PROJECT_MEMORY.md", memory)]).len(), 1);
}

#[test]
fn only_tracked_files_count_in_a_git_repo() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        dir.path(),
        &[
            ("src/a.py", ""),
            ("dist/JASE/x.txt", ""),
            ("docs/README.md", "`src/a.py` `dist/JASE/x.txt`"),
        ],
    );
    for args in [&["init", "-q"][..], &["add", "src", "docs"]] {
        let ok = Command::new("git")
            .args(args)
            .current_dir(dir.path())
            .status()
            .expect("git");
        assert!(ok.success(), "git {args:?}");
    }
    assert_eq!(
        check(dir.path()),
        vec!["docs/README.md:1: ruta inexistente: dist/JASE/x.txt"]
    );
}

#[test]
fn historical_docs_skip_path_check() {
    let line = "`src/falta.py` [x](falta.md) /home/ana\n";
    let found = problems(&[
        ("src/a.py", ""),
        ("docs/ai/PROJECT_MEMORY.md", line),
        ("docs/hoja-de-ruta/port-rust.md", line),
        ("docs/otro.md", line),
    ]);
    assert_eq!(
        found,
        vec![
            "docs/ai/PROJECT_MEMORY.md:1: enlace roto: falta.md",
            "docs/ai/PROJECT_MEMORY.md:1: dato personal (ruta /home)",
            "docs/hoja-de-ruta/port-rust.md:1: enlace roto: falta.md",
            "docs/hoja-de-ruta/port-rust.md:1: dato personal (ruta /home)",
            "docs/otro.md:1: enlace roto: falta.md",
            "docs/otro.md:1: ruta inexistente: src/falta.py",
            "docs/otro.md:1: dato personal (ruta /home)",
        ]
    );
}

const EXCLUDED: &str = "docs/superpowers/";
const MEMORY: &str = "docs/ai/PROJECT_MEMORY.md";
/// Registro histórico: nombra archivos de Python que ya no están; sus rutas entre comillas
/// invertidas no se revisan (sí enlaces, datos personales y etapas).
const HISTORICAL: &[&str] = &[
    "docs/ai/PROJECT_MEMORY.md",
    "docs/hoja-de-ruta/port-rust.md",
];
const DOC_RANGES: &[&str] = &["192.0.2.", "198.51.100.", "203.0.113."];
const SPECIAL_IPS: &[&str] = &["127.0.0.1", "0.0.0.0", "255.255.255.255"];

/// Problemas encontrados, como «ruta:línea: mensaje»; vacío = todo bien.
fn check(root: &Path) -> Vec<String> {
    let repo = Repo::new(root);
    let mut problems = Vec::new();
    for (path, rel) in docs(root) {
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{rel}: {e}"));
        let personal = rel == "AGENTS.md" || rel.starts_with("docs/");
        let historical = HISTORICAL.contains(&rel.as_str());
        for (n, line) in text.lines().enumerate() {
            let mut found = check_links(&path, line);
            if !historical {
                found.extend(check_paths(&repo, line));
            }
            if personal {
                found.extend(check_personal(line));
            }
            problems.extend(found.into_iter().map(|m| format!("{rel}:{}: {m}", n + 1)));
        }
        if rel == MEMORY {
            problems.extend(
                check_memory(&text)
                    .into_iter()
                    .map(|(n, m)| format!("{rel}:{n}: {m}")),
            );
        }
    }
    problems
}

/// Los archivos revisados con su ruta relativa (con `/`), en el orden de Python.
fn docs(root: &Path) -> Vec<(std::path::PathBuf, String)> {
    let mut files: Vec<_> = ["README.md", "README.es.md", "AGENTS.md"]
        .iter()
        .map(|n| (root.join(n), n.to_string()))
        .filter(|(p, _)| p.is_file())
        .collect();
    let mut nested = Vec::new();
    walk(root, &root.join("docs"), &mut nested);
    // Por segmentos, como `sorted()` de rutas en Python.
    nested.sort_by(|a, b| a.1.split('/').cmp(b.1.split('/')));
    files.extend(
        nested
            .into_iter()
            .filter(|(_, rel)| rel.ends_with(".md") && !rel.starts_with(EXCLUDED)),
    );
    files
}

/// Todos los archivos bajo `dir` (que no existe = ninguno), con su ruta relativa a `root`.
fn walk(root: &Path, dir: &Path, out: &mut Vec<(std::path::PathBuf, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, out);
        } else {
            let rel = path.strip_prefix(root).expect("bajo root");
            let rel: Vec<_> = rel.iter().map(|s| s.to_string_lossy()).collect();
            out.push((path.clone(), rel.join("/")));
        }
    }
}

/// Qué existe en el repo: los archivos versionados (`git ls-files`) si `root` es la raíz de
/// un repo git, para que `target/` o `dist/` locales no cuenten; si no, los del disco.
struct Repo {
    files: Vec<String>,
    /// Raíces del disco (también `target/`, `dist/`): una ruta ahí se revisa, y sin versionar
    /// no existe.
    roots: Vec<String>,
}

impl Repo {
    fn new(root: &Path) -> Self {
        let files = tracked(root).unwrap_or_else(|| {
            let mut all = Vec::new();
            walk(root, root, &mut all);
            all.into_iter().map(|(_, rel)| rel).collect()
        });
        let roots = std::fs::read_dir(root)
            .expect("raíz del repo")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| !n.starts_with('.') || n == ".github")
            .collect();
        Self { files, roots }
    }

    fn exists(&self, rel: &str) -> bool {
        if rel.contains('*') {
            return self.files.iter().any(|f| glob(rel, f));
        }
        let dir = format!("{rel}/");
        self.files.iter().any(|f| f == rel || f.starts_with(&dir))
    }
}

fn tracked(root: &Path) -> Option<Vec<String>> {
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let top = git(&["rev-parse", "--show-toplevel"])?;
    let same = Path::new(top.trim()).canonicalize().ok()? == root.canonicalize().ok()?;
    if !same {
        return None;
    }
    // `-z`: sin las comillas con que git escapa los nombres no ASCII.
    let out = git(&["ls-files", "-z"])?;
    Some(
        out.split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect(),
    )
}

/// `fnmatch` de Python con solo `*` (cruza `/`).
// shortcut: sin `?` ni `[…]`, que hoy ninguna doc usa; agregarlos si alguna los cita.
fn glob(pattern: &str, name: &str) -> bool {
    let mut parts = pattern.split('*');
    let first = parts.next().unwrap_or_default();
    let Some(mut rest) = name.strip_prefix(first) else {
        return false;
    };
    let parts: Vec<&str> = parts.collect();
    let Some((last, middle)) = parts.split_last() else {
        return rest.is_empty();
    };
    for part in middle {
        match rest.find(part) {
            Some(i) => rest = &rest[i + part.len()..],
            None => return false,
        }
    }
    rest.ends_with(last)
}

/// Destinos de `[x](dest)`, `[x](dest "título")` y `[x]: dest`.
fn link_targets(line: &str) -> Vec<&str> {
    let b = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(off) = line[i..].find('[') {
        let open = i + off;
        i = open + 1;
        let Some(close) = line[open..].find(']').map(|c| open + c) else {
            break;
        };
        if b.get(close + 1) != Some(&b'(') {
            continue;
        }
        let start = close + 2;
        let end = start
            + line[start..]
                .find(|c: char| c == ')' || c.is_whitespace())
                .unwrap_or(line.len() - start);
        if end == start {
            continue;
        }
        let tail = &line[end..];
        let after = if let Some(t) = tail.strip_prefix(')') {
            Some(t)
        } else {
            let t = tail.trim_start();
            (t.len() < tail.len())
                .then(|| t.strip_prefix('"'))
                .flatten()
                .and_then(|t| t.find('"').map(|q| &t[q + 1..]))
                .and_then(|t| t.strip_prefix(')'))
        };
        if let Some(after) = after {
            out.push(&line[start..end]);
            i = line.len() - after.len();
        }
    }
    // `^\s*\[[^\]]+\]:\s*(\S+)`
    let t = line.trim_start();
    if let Some(inner) = t.strip_prefix('[')
        && let Some(close) = inner.find(']').filter(|&c| c > 0)
        && let Some(dest) = inner[close + 1..].strip_prefix(':')
        && let Some(dest) = dest.split_whitespace().next()
    {
        out.push(dest);
    }
    out
}

fn check_links(path: &Path, line: &str) -> Vec<String> {
    let parent = path.parent().expect("padre");
    link_targets(line)
        .into_iter()
        .filter(|t| !is_scheme(t))
        .filter(|t| {
            let file = t.split('#').next().unwrap_or_default();
            !file.is_empty() && !parent.join(file).exists()
        })
        .map(|t| format!("enlace roto: {t}"))
        .collect()
}

/// `^[a-z][a-z0-9+.-]*:|^#`
fn is_scheme(target: &str) -> bool {
    if target.starts_with('#') {
        return true;
    }
    let Some((scheme, _)) = target.split_once(':') else {
        return false;
    };
    scheme.starts_with(|c: char| c.is_ascii_lowercase())
        && scheme
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "+.-".contains(c))
}

/// Tokens entre comillas invertidas, como `` `([^`\n]+)` `` de `re.findall`.
fn ticks(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else {
            break;
        };
        if close == 0 {
            rest = after;
            continue;
        }
        out.push(&after[..close]);
        rest = &after[close + 1..];
    }
    out
}

/// Sin el sufijo `:N` o `:N-M`.
fn strip_line_suffix(token: &str) -> &str {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    match token.rsplit_once(':') {
        Some((head, n))
            if digits(n)
                || n.split_once('-')
                    .is_some_and(|(a, b)| digits(a) && digits(b)) =>
        {
            head
        }
        _ => token,
    }
}

fn check_paths(repo: &Repo, line: &str) -> Vec<String> {
    let mut out = Vec::new();
    for token in ticks(line) {
        let token = strip_line_suffix(token.trim()).trim_end_matches('/');
        if token.contains(' ')
            || token.contains('<')
            || !token.contains('/')
            || format!("{token}/").starts_with(EXCLUDED)
        {
            continue;
        }
        let first = token.split('/').next().unwrap_or_default();
        if repo.roots.iter().any(|r| r == first) && !repo.exists(token) {
            out.push(format!("ruta inexistente: {token}"));
        }
    }
    out
}

fn check_personal(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let found = [
        (has_drive(line), "ruta de unidad"),
        (line.contains("/home/"), "ruta /home"),
        (line.contains("/mnt/"), "ruta /mnt"),
        (has_hex_key(line), "posible clave"),
    ];
    for (hit, what) in found {
        if hit {
            out.push(format!("dato personal ({what})"));
        }
    }
    for ip in ipv4s(line) {
        let octets_ok = ip
            .split('.')
            .all(|o| o.parse::<u16>().is_ok_and(|n| n <= 255));
        if octets_ok && !SPECIAL_IPS.contains(&ip) && !DOC_RANGES.iter().any(|r| ip.starts_with(r))
        {
            out.push(format!(
                "dato personal (IPv4 {ip}; usar 192.0.2.x, 198.51.100.x o 203.0.113.x)"
            ));
        }
    }
    out
}

/// `(?<![%\w>])[A-Za-z]:[\\/]`
fn has_drive(line: &str) -> bool {
    let chars: Vec<char> = line.chars().collect();
    chars.windows(3).enumerate().any(|(i, w)| {
        let before_ok = i == 0 || {
            let p = chars[i - 1];
            !(p == '%' || p == '>' || p == '_' || p.is_alphanumeric())
        };
        before_ok && w[0].is_ascii_alphabetic() && w[1] == ':' && (w[2] == '\\' || w[2] == '/')
    })
}

/// `(?<![0-9a-zA-Z])[0-9a-fA-F]{32,}(?![0-9a-zA-Z])`: una tira alfanumérica ASCII entera hex.
fn has_hex_key(line: &str) -> bool {
    line.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|run| run.len() >= 32 && run.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// `(?<![\d.])(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})(?!\d|\.\d)`, como `re.finditer`.
fn ipv4s(line: &str) -> Vec<&str> {
    let b = line.as_bytes();
    let digits_at = |i: usize| b[i..].iter().take_while(|c| c.is_ascii_digit()).count();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if !b[i].is_ascii_digit() || (i > 0 && (b[i - 1].is_ascii_digit() || b[i - 1] == b'.')) {
            i += 1;
            continue;
        }
        let mut end = i;
        let mut ok = true;
        for octet in 0..4 {
            let n = digits_at(end);
            if n == 0 || n > 3 {
                ok = false;
                break;
            }
            end += n;
            if octet < 3 {
                if b.get(end) != Some(&b'.') {
                    ok = false;
                    break;
                }
                end += 1;
            }
        }
        let dot_digit = b.get(end) == Some(&b'.') && b.get(end + 1).is_some_and(u8::is_ascii_digit);
        if ok && !dot_digit {
            out.push(&line[i..end]);
            i = end;
        } else {
            i += 1;
        }
    }
    out
}

/// `## AAAA-MM-DD — tema` con algo que no sea espacio después.
fn stage_heading_ok(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("## ") else {
        return false;
    };
    let date = rest.as_bytes();
    let shape = date.len() > 10
        && date[..10].iter().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                *c == b'-'
            } else {
                c.is_ascii_digit()
            }
        });
    shape
        && rest[10..]
            .strip_prefix(" — ")
            .is_some_and(|t| t.starts_with(|c: char| !c.is_whitespace()))
}

fn check_memory(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut stage: Option<(String, usize, usize)> = None;
    let close = |stage: &Option<(String, usize, usize)>, out: &mut Vec<(usize, String)>| {
        if let Some((name, start, bullets)) = stage
            && *bullets > 3
        {
            out.push((
                *start,
                format!("etapa con {bullets} viñetas (máximo 3): {name}"),
            ));
        }
    };
    for (n, line) in text.lines().enumerate() {
        if let Some(name) = line.strip_prefix("## ") {
            close(&stage, &mut out);
            stage = Some((name.trim().to_owned(), n + 1, 0));
            if !stage_heading_ok(line) {
                out.push((
                    n + 1,
                    "etapa sin el formato «## AAAA-MM-DD — tema»".to_owned(),
                ));
            }
        } else if let Some((_, _, bullets)) = stage.as_mut()
            && (line.starts_with("- ") || line.starts_with("* "))
        {
            *bullets += 1;
        }
    }
    close(&stage, &mut out);
    out
}
