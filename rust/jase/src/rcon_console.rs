//! Consola RCON de un servidor (`rcon_dialog.py`): clave, comando con historial y salida, sin
//! `App`. La ventana, el llavero y el envío viven en `app/rcon.rs`.

use std::sync::Arc;

use iced::widget::text_editor::{Action, Content, Edit, Motion};
use iced::widget::{Column as Col, Id, container, row, text, text_editor};
use iced::{Center, Element, Fill, Font, window};
use jase_core::i18n::{tr, tr_fmt};
use jase_core::pystr::py_strip;

use crate::app::{Message, RconMsg};
use crate::passwords::Secret;
use crate::theme::{framed, palette, style, widget_style};
use crate::widgets;

pub const HISTORY_MAX: usize = 50;
/// `setMaximumBlockCount(5000)`: las líneas viejas se descartan.
pub const RCON_MAX_LINES: usize = 5000;

/// El campo «Servidor:» de todas las consolas: nadie lo enfoca ni lo opera.
const ADDRESS_ID: Id = Id::new("rcon-address");

/// El texto de un código de `rcon::ERRORS`; el detalle (del sistema, nunca la clave) va entre
/// paréntesis.
pub fn error_text(code: &str, detail: &str) -> String {
    let known = match code {
        "timeout" => tr("El servidor no respondió al RCON."),
        "bad_password" => tr("Clave RCON incorrecta."),
        "no_challenge" => tr("El servidor no dio un challenge de RCON (¿tiene RCON activado?)."),
        "connect" => tr("No se pudo conectar al RCON (TCP) del servidor."),
        "invalid" => tr("Respuesta RCON inválida."),
        "closed" => tr("Se cortó la conexión RCON."),
        "unsupported" => tr("Este juego no tiene RCON en JASE."),
        _ => {
            let error = if detail.is_empty() { code } else { detail };
            return tr_fmt("Error de red: {error}", &[("error", &error)]);
        }
    };
    if detail.is_empty() {
        known
    } else {
        format!("{known} ({detail})")
    }
}

pub struct Console {
    /// El nombre del servidor al abrir (el título) y la dirección para conectar.
    pub server: String,
    pub address: String,
    pub password: Secret,
    pub remember: bool,
    /// Ya respondió el llavero: hasta entonces la clave y *Enviar* están apagados.
    pub loaded: bool,
    pub command: String,
    /// Un comando en camino: *Enviar* apagado.
    pub busy: bool,
    pub output: Content,
    history: Vec<String>,
    pos: usize,
    /// Sube con cada envío: la respuesta de otro se descarta.
    pub request: u64,
    pub password_field: Id,
    pub command_field: Id,
}

impl Console {
    pub fn new(server: String, address: String) -> Console {
        Console {
            server,
            address,
            password: Secret::default(),
            remember: false,
            loaded: false,
            command: String::new(),
            busy: false,
            output: Content::new(),
            history: Vec::new(),
            pos: 0,
            request: 0,
            // Las operaciones de foco llegan a todas las ventanas: ids propias.
            password_field: Id::unique(),
            command_field: Id::unique(),
        }
    }

    /// La clave guardada (o no); devuelve el campo que toma el foco: el comando si había clave.
    pub fn on_saved(&mut self, saved: Option<Secret>) -> Id {
        self.remember = saved.is_some();
        self.password = saved.unwrap_or_default();
        self.loaded = true;
        if self.password.0.is_empty() {
            self.password_field.clone()
        } else {
            self.command_field.clone()
        }
    }

    /// `_send`: clave, comando y «recordar», o `None` si no hay nada que mandar.
    pub fn send(&mut self) -> Option<(Secret, String, bool)> {
        let command = py_strip(&self.command).to_owned();
        if command.is_empty() || self.busy || !self.loaded {
            return None;
        }
        if self.history.last() != Some(&command) {
            self.history.push(command.clone());
            if self.history.len() > HISTORY_MAX {
                self.history.remove(0);
            }
        }
        self.pos = self.history.len();
        self.command.clear();
        self.append(&format!("> {command}"));
        self.busy = true;
        Some((self.password.clone(), command, self.remember))
    }

    /// ↑/↓ en el comando: el anterior o el siguiente; tras el último, vacío.
    pub fn history_step(&mut self, up: bool) {
        if self.history.is_empty() {
            return;
        }
        self.pos = if up {
            self.pos.saturating_sub(1)
        } else {
            (self.pos + 1).min(self.history.len())
        };
        self.command = self.history.get(self.pos).cloned().unwrap_or_default();
    }

    /// La respuesta tal cual (texto plano), sin los saltos de línea del final.
    pub fn show_answer(&mut self, text: &str) {
        let text = text.trim_end_matches('\n');
        if text.is_empty() {
            self.append(&tr("(sin respuesta de texto)"));
        } else {
            self.append(text);
        }
        self.busy = false;
    }

    pub fn show_error(&mut self, code: &str, detail: &str) {
        self.append(&format!("! {}", error_text(code, detail)));
        self.busy = false;
    }

    /// `appendPlainText` con el tope de líneas; la vista queda al final.
    fn append(&mut self, line: &str) {
        let text = if self.output.is_empty() {
            line.to_owned()
        } else {
            format!("\n{line}")
        };
        self.output.perform(Action::Move(Motion::DocumentEnd));
        self.output
            .perform(Action::Edit(Edit::Paste(Arc::new(text))));
        let extra = self.output.line_count().saturating_sub(RCON_MAX_LINES);
        if extra > 0 {
            let kept: Vec<String> = self
                .output
                .lines()
                .skip(extra)
                .map(|l| l.text.into_owned())
                .collect();
            self.output = Content::with_text(&kept.join("\n"));
            self.output.perform(Action::Move(Motion::DocumentEnd));
        }
    }

    /// «Servidor:» y «Clave RCON:» con *Recordar*, la salida monoespaciada de solo lectura, y el
    /// comando con *Enviar*.
    pub fn view(&self, window: window::Id, theme_id: &'static str) -> Element<'_, Message> {
        let (p, s) = (palette(theme_id), widget_style(theme_id));
        let msg = Message::Rcon;
        let label = |t: String| text(t).color(p.text).font(s.font()).size(s.text_size());
        let names = [tr("Servidor:"), tr("Clave RCON:")];
        let label_w = names
            .iter()
            .map(|n| widgets::text_width(n, s.font(), s.text_size()))
            .fold(0.0, f32::max);
        let [server_name, key_name] = names;
        let no_input = None::<fn(String) -> Message>;
        let address = widgets::line_edit(
            ADDRESS_ID,
            "",
            &self.address,
            no_input,
            None,
            false,
            false,
            theme_id,
        );
        let send = (self.loaded && !self.busy).then_some(msg(RconMsg::Send(window)));
        let password = widgets::line_edit(
            self.password_field.clone(),
            "",
            &self.password.0,
            self.loaded
                .then_some(move |t| msg(RconMsg::Password(window, Secret(t)))),
            send.clone(),
            false,
            true,
            theme_id,
        );
        let remember = widgets::tip(
            widgets::check(
                self.remember,
                Some(label(tr("Recordar la clave")).into()),
                self.loaded
                    .then_some(msg(RconMsg::Remember(window, !self.remember))),
                p,
                s,
            ),
            tr("Se guarda en el Administrador de credenciales"),
            p,
            s,
        );
        let form = Col::new()
            .push(
                row![label(server_name).width(label_w), address]
                    .spacing(6)
                    .align_y(Center),
            )
            .push(
                row![label(key_name).width(label_w), password, remember]
                    .spacing(6)
                    .align_y(Center),
            )
            .spacing(6);
        // Solo lectura: se aplican mover, elegir y desplazar; nunca una edición.
        let editor = text_editor(&self.output)
            .on_action(move |a| msg(RconMsg::Output(window, a)))
            .font(Font::MONOSPACE)
            .size(s.text_size())
            .padding(widgets::field_padding(s))
            .height(Fill)
            .style(move |_, status| {
                let focused = matches!(status, text_editor::Status::Focused { .. });
                style::text_editor(p, s, theme_id, focused)
            });
        let (_, _, edges, _) = style::field(p, s, theme_id, false);
        let output = match edges {
            Some(e) => framed(editor, e),
            None => editor.into(),
        };
        let command = widgets::field(
            self.command_field.clone(),
            &tr("Comando (p. ej. status, map mp_toujane)"),
            &self.command,
            move |t| msg(RconMsg::Command(window, t)),
            send.clone(),
            false,
            theme_id,
        );
        let command = row![command, widgets::button_maybe(tr("Enviar"), send, theme_id)]
            .spacing(6)
            .align_y(Center);
        container(Col::new().push(form).push(output).push(command).spacing(6))
            .padding(10)
            .width(Fill)
            .height(Fill)
            .style(move |_| style::window(p))
            .into()
    }
}
