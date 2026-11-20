//! «Agregar servidor…»: direcciones en texto libre y el formulario (como `ui/add_server.py`).

use std::fmt;
use std::sync::Arc;

use iced::widget::{Id, Space, column, row, text, text_editor};
use iced::{Center, Element, Fill, Top};
use jase_core::games::{GameDef, game};
use jase_core::i18n::{tr, tr_fmt, tr_n};
use jase_core::jase_url;
use jase_core::masters::valid_host;
use jase_core::models::{ServerAddress, ServerInfo};
use jase_core::pystr::py_strip;

use crate::app::Message;
use crate::app::dialogs::card;
use crate::theme::{palette, style, widget_style};
use crate::widgets::{self, text_width};

/// Por pegado: una lista de un clan, no un volcado de un master.
pub const MAX_ADDRESSES: usize = 100;
/// Lo que se analiza de un texto: llega del portapapeles y se mira en cada tecla de la lista.
pub const MAX_PASTE_BYTES: usize = 65_536;
/// Caracteres del campo IP (`QLineEdit::maxLength` por defecto).
const MAX_HOST_CHARS: usize = 32_767;

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

/// `\w` de Python sobre `str`. Divergencia: `is_alphanumeric` incluye Other_Alphabetic
/// (marcas combinantes como «ा», letras en círculo como «Ⓐ») y `isalnum` no: junto a ellas
/// Rust rechaza de más, nunca acepta de más.
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Direcciones «host:puerto» de un texto cualquiera, sin repetir y como mucho
/// `MAX_ADDRESSES` (el host lleva al menos un punto: IP o dominio). Equivale al regex
/// `(?<![\w.:-])([A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+):(\d{1,5})(?![\w.:])` de Python: el
/// host solo puede ser un tramo máximo de `[A-Za-z0-9.-]` (lo de antes no puede ser de
/// esos caracteres y lo de después es `:`). Solo mira los primeros `MAX_PASTE_BYTES`.
pub fn find_addresses(text: &str) -> Vec<ServerAddress> {
    let text = head(text);
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

/// Los primeros `MAX_PASTE_BYTES` del texto; si el corte cae dentro de un tramo de dirección,
/// sin ese tramo, que pudo quedar a medias (`…:28960` → `…:2896`, válida pero equivocada).
fn head(text: &str) -> &str {
    if text.len() <= MAX_PASTE_BYTES {
        return text;
    }
    let mut cut = MAX_PASTE_BYTES;
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    let in_span = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '-');
    if text[cut..].starts_with(in_span) {
        text[..cut].trim_end_matches(in_span)
    } else {
        &text[..cut]
    }
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
    pub fn host_changed(&mut self, mut text: String) {
        // Como el tope de `QLineEdit` en Qt: un pegado enorme no entra entero.
        if let Some((end, _)) = text.char_indices().nth(MAX_HOST_CHARS) {
            text.truncate(end);
        }
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
        let action = match action {
            // El editor da forma a todo el texto en `update`: un pegado de megas no entra.
            text_editor::Action::Edit(text_editor::Edit::Paste(t)) => {
                let mut end = MAX_PASTE_BYTES
                    .saturating_sub(self.list.text().len())
                    .min(t.len());
                while !t.is_char_boundary(end) {
                    end -= 1;
                }
                let t = if end < t.len() {
                    Arc::new(t[..end].to_owned())
                } else {
                    t
                };
                text_editor::Action::Edit(text_editor::Edit::Paste(t))
            }
            action => action,
        };
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
        let text = head(text); // un portapapeles de megas no entra entero en el editor
        self.list = text_editor::Content::with_text(text);
        self.found = find_addresses(text);
    }

    /// Varias direcciones → modo lista; un enlace jase:// (elige también el juego),
    /// «IP:puerto» o texto con una sola dirección → casillas de IP y puerto.
    pub fn apply_text(&mut self, text: &str) {
        let found = find_addresses(text);
        if found.len() > 1 {
            self.host.clear();
            self.set_list(py_strip(head(text)));
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

/// Los campos del diálogo (el foco va al de `Form::focus`).
pub const HOST_ID: Id = Id::new("add-server-host");
pub const PORT_ID: Id = Id::new("add-server-port");
pub const LIST_ID: Id = Id::new("add-server-list");

pub fn field_id(field: Field) -> Id {
    match field {
        Field::Host => HOST_ID,
        Field::Port => PORT_ID,
        Field::List => LIST_ID,
    }
}

/// Un juego del combo, con su nombre.
#[derive(Clone, PartialEq)]
struct GameOption(&'static str);

impl fmt::Display for GameOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(game(self.0).map_or(self.0, |g| g.name))
    }
}

/// `AddServerDialog`: el `QFormLayout` (juego y *Detectar*, IP y puerto o la lista), la
/// etiqueta de detección, la casilla y los botones (*Varias direcciones…* a la izquierda,
/// *Agregar*/*Cancelar* a la derecha, como el `QDialogButtonBox` de Windows).
pub fn view<'a>(form: &'a Form, theme_id: &'static str) -> Element<'a, Message> {
    let (p, s) = (palette(theme_id), widget_style(theme_id));
    let label = |t: String| text(t).font(s.font()).size(s.text_size());
    let msg = Message::AddServer;
    let names = [tr("Juego:"), tr("IP:"), tr("Puerto:"), tr("Direcciones:")];
    // Como `QFormLayout`: la columna de etiquetas mide la más ancha de las filas visibles.
    let shown: &[usize] = if form.multi { &[0, 3] } else { &[0, 1, 2] };
    let label_w = shown
        .iter()
        .map(|&i| text_width(&names[i], s.font(), s.text_size()))
        .fold(0.0, f32::max);
    let [game_name, ip_name, port_name, list_name] = names;
    let form_row = |name: String, field: Element<'a, Message>| {
        row![label(name).width(label_w), field]
            .spacing(6)
            .align_y(Center)
    };
    let games = form.games.iter().map(|&id| GameOption(id)).collect();
    let combo = widgets::combo(
        games,
        Some(GameOption(form.game)),
        move |o: GameOption| msg(Msg::Game(o.0)),
        None,
        true,
        theme_id,
    );
    let detect = widgets::tip(
        widgets::button_maybe(
            tr("Detectar"),
            form.can_detect().then_some(msg(Msg::Detect)),
            theme_id,
        ),
        tr("Consulta la dirección con cada protocolo y elige el juego que responde"),
        p,
        s,
    );
    // El espaciado de 6 de `QFormLayout` y `QVBoxLayout`; el título va aparte con el de la tarjeta.
    let mut content = column![form_row(
        game_name,
        row![combo, detect].spacing(6).align_y(Center).into()
    )]
    .spacing(6);
    if form.multi {
        let list = widgets::text_area(
            LIST_ID,
            &tr("Pega aquí varias direcciones IP:puerto (una por línea o mezcladas con texto)"),
            &form.list,
            move |a| msg(Msg::List(a)),
            // El `sizeHint` de `QAbstractScrollArea` (256×192), igual en todos los estilos.
            192.0,
            form.focus == Field::List,
            theme_id,
        );
        content = content.push(
            row![label(list_name).width(label_w), list]
                .spacing(6)
                .align_y(Top),
        );
    } else {
        let field = |id, placeholder: String, value: &str, on: fn(String) -> Msg, focused| {
            widgets::field(
                id,
                &placeholder,
                value,
                move |t| msg(on(t)),
                Some(msg(Msg::HostSubmit)),
                focused,
                theme_id,
            )
        };
        let host = field(
            HOST_ID,
            tr("IP o nombre (se puede pegar IP:puerto)"),
            &form.host,
            Msg::Host,
            form.focus == Field::Host,
        );
        let port = field(
            PORT_ID,
            tr("Puerto"),
            &form.port,
            Msg::Port,
            form.focus == Field::Port,
        );
        content = content
            .push(form_row(ip_name, host))
            .push(form_row(port_name, port));
    }
    let favorite = widgets::check(
        form.favorite,
        Some(label(tr("Agregar a favoritos")).into()),
        Some(msg(Msg::Favorite(!form.favorite))),
        p,
        s,
    );
    let multi = if form.multi {
        tr("Una dirección")
    } else {
        tr("Varias direcciones…")
    };
    let buttons = row![
        widgets::button(multi, msg(Msg::ToggleMulti), theme_id),
        Space::new().width(Fill),
        widgets::button_maybe(
            form.ok_label(),
            form.ok_enabled().then_some(msg(Msg::Submit)),
            theme_id
        ),
        widgets::button(tr("Cancelar"), msg(Msg::Cancel), theme_id),
    ]
    .spacing(6);
    let content = content
        .push(form.detect_label.clone().map(label))
        .push(favorite)
        .push(buttons);
    let title = label(tr("Agregar servidor")).font(style::header_font(s));
    card(column![title, content], 420.0, theme_id)
}
