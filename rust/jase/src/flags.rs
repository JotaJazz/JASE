//! Banderas (flag-icons 4x3, MIT) embebidas desde `src/jase/assets/flags/`.

use std::sync::LazyLock;

use iced::Size;
use iced::widget::svg::Handle;

pub const FLAG_SIZE: Size = Size::new(20.0, 15.0);

macro_rules! flags {
    ($($code:literal),* $(,)?) => {
        /// Ordenadas por código para la búsqueda binaria.
        static FLAGS: &[(&str, &[u8])] = &[$((
            $code,
            include_bytes!(concat!("../../../src/jase/assets/flags/", $code, ".svg")),
        )),*];
    };
}

flags![
    "ad", "ae", "af", "ag", "ai", "al", "am", "ao", "aq", "ar", "as", "at", "au", "aw", "ax", "az",
    "ba", "bb", "bd", "be", "bf", "bg", "bh", "bi", "bj", "bl", "bm", "bn", "bo", "bq", "br", "bs",
    "bt", "bv", "bw", "by", "bz", "ca", "cc", "cd", "cf", "cg", "ch", "ci", "ck", "cl", "cm", "cn",
    "co", "cp", "cr", "cu", "cv", "cw", "cx", "cy", "cz", "de", "dg", "dj", "dk", "dm", "do", "dz",
    "ec", "ee", "eg", "eh", "er", "es", "et", "eu", "fi", "fj", "fk", "fm", "fo", "fr", "ga", "gb",
    "gd", "ge", "gf", "gg", "gh", "gi", "gl", "gm", "gn", "gp", "gq", "gr", "gs", "gt", "gu", "gw",
    "gy", "hk", "hm", "hn", "hr", "ht", "hu", "ic", "id", "ie", "il", "im", "in", "io", "iq", "ir",
    "is", "it", "je", "jm", "jo", "jp", "ke", "kg", "kh", "ki", "km", "kn", "kp", "kr", "kw", "ky",
    "kz", "la", "lb", "lc", "li", "lk", "lr", "ls", "lt", "lu", "lv", "ly", "ma", "mc", "md", "me",
    "mf", "mg", "mh", "mk", "ml", "mm", "mn", "mo", "mp", "mq", "mr", "ms", "mt", "mu", "mv", "mw",
    "mx", "my", "mz", "na", "nc", "ne", "nf", "ng", "ni", "nl", "no", "np", "nr", "nu", "nz", "om",
    "pa", "pc", "pe", "pf", "pg", "ph", "pk", "pl", "pm", "pn", "pr", "ps", "pt", "pw", "py", "qa",
    "re", "ro", "rs", "ru", "rw", "sa", "sb", "sc", "sd", "se", "sg", "sh", "si", "sj", "sk", "sl",
    "sm", "sn", "so", "sr", "ss", "st", "sv", "sx", "sy", "sz", "tc", "td", "tf", "tg", "th", "tj",
    "tk", "tl", "tm", "tn", "to", "tr", "tt", "tv", "tw", "tz", "ua", "ug", "um", "un", "us", "uy",
    "uz", "va", "vc", "ve", "vg", "vi", "vn", "vu", "wf", "ws", "xk", "xx", "ye", "yt", "za", "zm",
    "zw"
];

/// SVG de la bandera del código ISO alfa-2 (como `flag_path`: 2 letras ASCII, sin importar
/// mayúsculas ni espacios alrededor); `None` si no es válido o no hay bandera.
pub fn flag_svg(code: &str) -> Option<&'static [u8]> {
    flag_index(code).map(|i| FLAGS[i].1)
}

fn flag_index(code: &str) -> Option<usize> {
    let code = code.trim();
    if code.len() != 2 || !code.bytes().all(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    let code = code.to_ascii_lowercase();
    FLAGS
        .binary_search_by(|(c, _)| (*c).cmp(code.as_str()))
        .ok()
}

/// `Handle` de la bandera; se arman una vez (`from_memory` hashea los bytes) y la tabla los
/// clona por fila.
pub fn flag(code: &str) -> Option<Handle> {
    static HANDLES: LazyLock<Vec<Handle>> = LazyLock::new(|| {
        FLAGS
            .iter()
            .map(|(_, svg)| Handle::from_memory(*svg))
            .collect()
    });
    flag_index(code).map(|i| HANDLES[i].clone())
}

pub fn codes() -> impl Iterator<Item = &'static str> {
    FLAGS.iter().map(|(c, _)| *c)
}
