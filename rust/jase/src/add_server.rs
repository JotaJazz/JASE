//! «Agregar servidor…»: direcciones en texto libre y el formulario (como `ui/add_server.py`).

use iced::widget::text_editor;
use jase_core::games::{GameDef, game};
use jase_core::i18n::{tr, tr_fmt, tr_n};
use jase_core::jase_url;
use jase_core::masters::valid_host;
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::pystr::py_strip;

/// Por pegado: una lista de un clan, no un volcado de un master.
pub const MAX_ADDRESSES: usize = 100;
/// Lo que se analiza de un texto: llega del portapapeles y se mira en cada tecla de la lista.
pub const MAX_PASTE_BYTES: usize = 65_536;

/// `"1.2.3.4:28960"` → ("1.2.3.4", 28960); sin puerto válido, (texto limpio, None).
pub fn split_host_port(text: &str) -> (String, Option<u16>) {
    let text = py_strip(text);
    if let Some((host, port)) = text.rsplit_once(':') {
        let digits = !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit());
        if let Some(port) = digits.then(|| port_number(port)).flatten() {
            return (py_strip(host).to_owned(), Some(port));
        }
    }
    (text.to_owned(), None)
}

/// Dígitos ASCII → puerto 1–65535.
fn port_number(digits: &str) -> Option<u16> {
    digits
        .parse::<u32>()
        .ok()
        .and_then(|p| u16::try_from(p).ok())
        .filter(|&p| p >= 1)
}

/// `\w` de Python sobre `str`.
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Direcciones «host:puerto» de un texto cualquiera, sin repetir y como mucho
/// `MAX_ADDRESSES` (el host lleva al menos un punto: IP o dominio). Equivale al regex
/// `(?<![\w.:-])([A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+):(\d{1,5})(?![\w.:])` de Python: el
/// host solo puede ser un tramo máximo de `[A-Za-z0-9.-]` (lo de antes no puede ser de
/// esos caracteres y lo de después es `:`). Solo mira los primeros `MAX_PASTE_BYTES`.
pub fn find_addresses(text: &str) -> Vec<ServerAddress> {
    let mut cut = text.len().min(MAX_PASTE_BYTES);
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    let text = &text[..cut];
    let in_run = |b: u8| b.is_ascii_alphanumeric() || b == b'.' || b == b'-';
    let bytes = text.as_bytes();
    let mut result = Vec::new();
    let mut i = 0;
    while i < bytes.len() && result.len() < MAX_ADDRESSES {
        if !in_run(bytes[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && in_run(bytes[i]) {
            i += 1;
        }
        if let Some(address) = address_at(text, start, i)
            && !result.contains(&address)
        {
            result.push(address);
        }
    }
    result
}

/// El tramo `text[start..end]` seguido de `:puerto`, si cumple el regex y es válido.
fn address_at(text: &str, start: usize, end: usize) -> Option<ServerAddress> {
    if text[..start]
        .chars()
        .next_back()
        .is_some_and(|c| c == ':' || is_word(c))
    {
        return None;
    }
    let host = &text[start..end];
    if !host.contains('.') || host.split('.').any(str::is_empty) || host.starts_with('-') {
        return None;
    }
    let rest = text[end..].strip_prefix(':')?;
    // Divergencia: el `\d` de Python también acepta dígitos no ASCII («٢٨»).
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if !(1..=5).contains(&digits)
        || rest[digits..]
            .chars()
            .next()
            .is_some_and(|c| c == '.' || c == ':' || is_word(c))
    {
        return None;
    }
    let port = port_number(&rest[..digits])?;
    valid_host(host).then(|| ServerAddress::new(host, port))
}

/// Direcciones a consultar: la escrita y, en GameSpy, la de consulta que corresponde si
/// el usuario escribió el puerto de juego (el inverso de `connect_port_fallback`).
pub fn query_candidates(game: &GameDef, port: u16, host: &str) -> Vec<ServerAddress> {
    let mut result = vec![ServerAddress::new(host, port)];
    let query_port = i32::from(port) - game.connect_port_fallback;
    if game.connect_port_fallback != 0
        && let Some(p) = u16::try_from(query_port).ok().filter(|&p| p >= 1)
    {
        result.push(ServerAddress::new(host, p));
    }
    result
}

/// El texto trae algo que agregar: una dirección o un enlace `jase://`.
pub fn has_address(text: &str) -> bool {
    !find_addresses(text).is_empty() || jase_url::parse(py_strip(text)).is_some()
}

/// El campo que se dibuja enfocado: el último editado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Host,
    Port,
    List,
}

/// El estado del diálogo, sin ventana ni E/S (`Content` no es `Eq`: vive fuera de `Dialog`).
pub struct Form {
    games: Vec<&'static str>,
    game: &'static str,
    host: String,
    port: String,
    list: text_editor::Content,
    /// Lo que da la lista; solo se recalcula cuando cambia (`view` nunca analiza texto).
    found: Vec<ServerAddress>,
    multi: bool,
    favorite: bool,
    detect_label: Option<String>,
    detecting: bool,
    focus: Field,
}

impl Form {
    /// `text`: contenido inicial (p. ej. el portapapeles).
    pub fn new(
        games: Vec<&'static str>,
        current: Option<&str>,
        favorite: bool,
        text: &str,
    ) -> Form {
        let game = current
            .and_then(|c| games.iter().find(|&&g| g == c))
            .or(games.first())
            .copied()
            .unwrap_or_default();
        let mut form = Form {
            games,
            game,
            host: String::new(),
            port: String::new(),
            list: text_editor::Content::new(),
            found: Vec::new(),
            multi: false,
            favorite,
            detect_label: None,
            detecting: false,
            focus: Field::Host,
        };
        if !text.is_empty() {
            form.apply_text(text);
        }
        form
    }

    pub fn game(&self) -> &'static str {
        self.game
    }

    pub fn set_game(&mut self, id: &'static str) {
        self.game = id;
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> &str {
        &self.port
    }

    /// Al pegar (entra más de un carácter de golpe) reparte el texto en sus casillas;
    /// tecla a tecla se separa con Enter.
    pub fn host_changed(&mut self, text: String) {
        let pasted = text.chars().count() > self.host.chars().count() + 1;
        self.host = text;
        self.focus = Field::Host;
        if pasted {
            self.apply_text(&self.host.clone());
        }
    }

    pub fn host_submitted(&mut self) {
        self.apply_text(&self.host.clone());
    }

    /// Solo hasta 5 dígitos (`QIntValidator` en Qt); lo demás no entra.
    pub fn port_changed(&mut self, text: String) {
        self.focus = Field::Port;
        if text.len() <= 5 && text.bytes().all(|b| b.is_ascii_digit()) {
            self.port = text;
        }
    }

    pub fn list_action(&mut self, action: text_editor::Action) {
        self.focus = Field::List;
        let edit = action.is_edit();
        self.list.perform(action);
        if edit {
            self.found = find_addresses(&self.list.text());
        }
    }

    pub fn list_text(&self) -> String {
        self.list.text()
    }

    /// El `Content` para el `text_editor` de la vista.
    pub fn list(&self) -> &text_editor::Content {
        &self.list
    }

    fn set_list(&mut self, text: &str) {
        self.list = text_editor::Content::with_text(text);
        self.found = find_addresses(text);
    }

    /// Varias direcciones → modo lista; un enlace jase:// (elige también el juego),
    /// «IP:puerto» o texto con una sola dirección → casillas de IP y puerto.
    pub fn apply_text(&mut self, text: &str) {
        let found = find_addresses(text);
        if found.len() > 1 {
            self.host.clear();
            self.set_list(py_strip(text));
            self.set_multi(true);
            return;
        }
        let (host, port) = if let Some((id, address)) = jase_url::parse(py_strip(text)) {
            if self.games.contains(&id) {
                self.game = id;
            }
            (address.host, Some(address.port))
        } else if let Some(first) = found.into_iter().next() {
            (first.host, Some(first.port))
        } else {
            split_host_port(text)
        };
        if let Some(port) = port {
            self.port = port.to_string();
        }
        if host != text {
            self.host = host;
        }
    }

    pub fn is_multi(&self) -> bool {
        self.multi
    }

    /// Al pasar a lista sin nada en ella, lo ya escrito pasa a ser la primera línea.
    pub fn set_multi(&mut self, multi: bool) {
        self.multi = multi;
        if multi
            && py_strip(&self.list.text()).is_empty()
            && let Some(single) = self.result_address()
        {
            self.set_list(&format!("{single}\n"));
        }
        self.focus = if multi { Field::List } else { Field::Host };
    }

    pub fn favorite(&self) -> bool {
        self.favorite
    }

    pub fn set_favorite(&mut self, on: bool) {
        self.favorite = on;
    }

    /// Dirección escrita; «IP:puerto» aún sin separar en la casilla de IP también vale.
    pub fn result_address(&self) -> Option<ServerAddress> {
        let (host, inline_port) = split_host_port(&self.host);
        let port = match inline_port {
            Some(port) => port,
            None if !self.port.is_empty() && self.port.bytes().all(|b| b.is_ascii_digit()) => {
                port_number(&self.port)?
            }
            None => return None,
        };
        (valid_host(&host) && !host.starts_with(['.', '-'])).then(|| ServerAddress::new(host, port))
    }

    /// Lo que se agregará: las direcciones de la lista o la de las casillas.
    pub fn addresses(&self) -> Vec<ServerAddress> {
        if self.multi {
            self.found.clone()
        } else {
            self.result_address().into_iter().collect()
        }
    }

    fn count(&self) -> usize {
        if self.multi {
            self.found.len()
        } else {
            usize::from(self.result_address().is_some())
        }
    }

    pub fn ok_label(&self) -> String {
        let n = self.count();
        if self.multi && n > 0 {
            tr_n(
                "Agregar 1 servidor",
                "Agregar {n} servidores",
                n as i64,
                &[],
            )
        } else {
            tr("Agregar")
        }
    }

    pub fn ok_enabled(&self) -> bool {
        self.count() > 0
    }

    /// «Detectar» es para una sola dirección y una detección a la vez.
    pub fn can_detect(&self) -> bool {
        !self.multi && self.count() == 1 && !self.detecting
    }

    pub fn start_detect(&mut self, address: &ServerAddress) {
        self.detecting = true;
        self.detect_label = Some(tr_fmt(
            "Detectando el juego de {address}…",
            &[("address", address)],
        ));
    }

    /// `games`: `(id, dirección de consulta)` de los juegos que respondieron.
    pub fn on_detected(
        &mut self,
        address: &ServerAddress,
        games: &[(&'static str, ServerAddress)],
    ) {
        self.detecting = false;
        let games: Vec<_> = games
            .iter()
            .filter(|(id, _)| self.games.contains(id))
            .collect();
        let Some((id, query_address)) = games.first() else {
            self.detect_label = Some(tr_fmt(
                "{address} no respondió como ningún juego conocido",
                &[("address", address)],
            ));
            return;
        };
        self.game = id;
        if query_address.host == address.host {
            self.port = query_address.port.to_string(); // p. ej. el de consulta de GameSpy
        }
        let name = |id: &str| game(id).map_or("", |g| g.name);
        self.detect_label = Some(if games.len() == 1 {
            tr_fmt("Detectado: {game}", &[("game", &name(id))])
        } else {
            let names: Vec<&str> = games.iter().map(|(id, _)| name(id)).collect();
            tr_fmt("Puede ser: {games}", &[("games", &names.join(", "))])
        });
    }

    pub fn detect_label(&self) -> Option<&str> {
        self.detect_label.as_deref()
    }

    pub fn focus(&self) -> Field {
        self.focus
    }
}

/// Mensajes del diálogo y de lo que agrega (la app los conecta).
#[allow(clippy::large_enum_variant)] // `ServerInfo`, como `DirectMsg`
#[derive(Debug, Clone)]
pub enum Msg {
    Open {
        favorite: bool,
        text: String,
    },
    Pasted(Option<String>),
    Game(&'static str),
    Host(String),
    HostSubmit,
    Port(String),
    List(text_editor::Action),
    ToggleMulti,
    Favorite(bool),
    Detect,
    Detected {
        request: u64,
        address: ServerAddress,
        games: Vec<(&'static str, ServerAddress)>,
    },
    Submit,
    Cancel,
    Found {
        request: u64,
        address: ServerAddress,
        result: Result<Option<ServerInfo>, String>,
    },
    Finished {
        request: u64,
    },
}
