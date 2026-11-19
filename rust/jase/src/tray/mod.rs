//! El icono de la bandeja y sus globos (`QSystemTrayIcon`), visto desde la app: lo que la
//! app le pide (`TrayHandle`) y lo que le llega (`TrayEvent`). El backend corre en el stream
//! de una suscripción: crear o quitar el icono y mostrar un globo nunca pasan en `update`.

use std::fmt;
use std::sync::Arc;

use futures::StreamExt;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use futures::stream::BoxStream;

/// Crea el icono con esos textos de menú y trae sus eventos; al soltarse el stream, lo quita.
pub type TrayFn = fn(&TrayLabels) -> BoxStream<'static, TrayEvent>;

/// Los textos del menú del icono (otro idioma = otro icono).
#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct TrayLabels {
    pub show: String,
    pub quit: String,
}

#[derive(Debug, Clone)]
pub enum TrayEvent {
    /// El icono ya está: por aquí se le habla.
    Ready(TrayHandle),
    /// Clic en el icono o *Mostrar JASE*.
    Show,
    /// *Salir*.
    Quit,
    /// Clic en el último globo.
    MessageClicked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayCommand {
    Notify { title: String, text: String },
}

#[derive(Clone)]
pub struct TrayHandle {
    /// La bandeja muestra globos (`supportsMessages`).
    pub messages: bool,
    tx: UnboundedSender<TrayCommand>,
    shutdown: Arc<dyn Fn() + Send + Sync>,
}

impl fmt::Debug for TrayHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TrayHandle")
            .field("messages", &self.messages)
            .finish_non_exhaustive()
    }
}

impl TrayHandle {
    /// Para los backends (y los tests): los comandos llegan por el receptor.
    pub fn new(
        messages: bool,
        shutdown: Arc<dyn Fn() + Send + Sync>,
    ) -> (TrayHandle, UnboundedReceiver<TrayCommand>) {
        let (tx, rx) = unbounded();
        let handle = TrayHandle {
            messages,
            tx,
            shutdown,
        };
        (handle, rx)
    }

    /// Solo encola: el globo lo muestra el backend.
    pub fn notify(&self, title: &str, text: &str) {
        let _ = self.tx.unbounded_send(TrayCommand::Notify {
            title: title.to_owned(),
            text: text.to_owned(),
        });
    }

    /// Quita el icono ya (al salir con `process::exit` no quedaría el fantasma).
    pub fn shutdown(&self) {
        (self.shutdown)();
    }
}

/// Sin backend todavía (Task 7): nunca hay icono.
pub fn run(_labels: &TrayLabels) -> BoxStream<'static, TrayEvent> {
    futures::stream::empty().boxed()
}

/// El cuerpo de una notificación de Linux admite marcado: el texto remoto va escapado.
pub fn escape_markup(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// `s` en UTF-16 con a lo sumo `max_units` unidades (sin partir un par sustituto) y el 0 final.
pub fn wide_truncated(s: &str, max_units: usize) -> Vec<u16> {
    let mut out = Vec::new();
    for c in s.chars() {
        if out.len() + c.len_utf16() > max_units {
            break;
        }
        let mut buf = [0; 2];
        out.extend_from_slice(c.encode_utf16(&mut buf));
    }
    out.push(0);
    out
}

/// RGBA → ARGB (el orden de bytes de `icon_pixmap` de StatusNotifierItem).
pub fn rgba_to_argb(rgba: &[u8]) -> Vec<u8> {
    let (pixels, _) = rgba.as_chunks::<4>();
    pixels
        .iter()
        .flat_map(|&[r, g, b, a]| [a, r, g, b])
        .collect()
}
