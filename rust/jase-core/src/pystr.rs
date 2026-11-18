//! Equivalentes de las operaciones de texto de Python que usan los parsers, para que
//! Rust dé exactamente el mismo resultado sobre los mismos bytes.

/// `bytes.decode("latin-1")`: cada byte es el `char` de ese código.
pub fn latin1(data: &[u8]) -> String {
    data.iter().map(|&b| char::from(b)).collect()
}

/// `raw.decode("utf-8")`, y si falla, latin-1.
pub fn utf8_or_latin1(data: &[u8]) -> String {
    match std::str::from_utf8(data) {
        Ok(s) => s.to_owned(),
        Err(_) => latin1(data),
    }
}

/// `str.isspace()`: Unicode más los separadores \x1c-\x1f, que Python cuenta.
pub fn is_py_space(c: char) -> bool {
    c.is_whitespace() || ('\x1c'..='\x1f').contains(&c)
}

/// `s.split()` sin argumentos.
pub fn py_split(s: &str) -> Vec<&str> {
    s.split(is_py_space).filter(|w| !w.is_empty()).collect()
}

/// `s.strip()` sin argumentos.
pub fn py_strip(s: &str) -> &str {
    s.trim_matches(is_py_space)
}

/// `c.isdigit()` dentro de latin-1: ASCII y los superíndices ² ³ ¹.
pub fn is_py_digit(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '\u{b2}' | '\u{b3}' | '\u{b9}')
}

/// `s.casefold()`.
// shortcut: solo cubre latin-1 (ß → ss), que es lo que llega de los servidores; usar
// un crate de case folding completo si entra texto de otros alfabetos.
pub fn casefold(s: &str) -> String {
    s.to_lowercase().replace('ß', "ss")
}

/// `s.encode("latin-1", "replace")`: lo que no cabe en latin-1 pasa a `?`.
pub fn latin1_encode_lossy(s: &str) -> Vec<u8> {
    s.chars().map(|c| u8::try_from(c).unwrap_or(b'?')).collect()
}

/// `s.splitlines()`: corta en los mismos separadores que Python, `\r\n` cuenta como uno.
pub fn py_splitlines(s: &str) -> Vec<&str> {
    let is_break = |c| {
        matches!(
            c,
            '\n' | '\r'
                | '\x0b'
                | '\x0c'
                | '\x1c'
                | '\x1d'
                | '\x1e'
                | '\u{85}'
                | '\u{2028}'
                | '\u{2029}'
        )
    };
    let mut lines = Vec::new();
    let mut rest = s;
    while let Some(i) = rest.find(is_break) {
        lines.push(&rest[..i]);
        let end = if rest[i..].starts_with("\r\n") {
            i + 2
        } else {
            i + rest[i..].chars().next().map_or(1, char::len_utf8)
        };
        rest = &rest[end..];
    }
    if !rest.is_empty() {
        lines.push(rest);
    }
    lines
}
