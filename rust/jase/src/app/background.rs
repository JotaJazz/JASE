//! Tareas de fondo (`window_background.py`): la lista oficial de masters, como mucho cada
//! 24 h salvo a mano desde Ajustes. Leer el `.meta.json` y bajar el archivo van en un hilo.

use std::path::PathBuf;
use std::time::Duration;

use iced::Task;
use jase_core::i18n::{tr, tr_fmt};
use jase_core::masters::{META_FILENAME, OfficialList, check_official, should_check};

use super::{App, Message, blocking};
use crate::masters_tab::Outcome;
use crate::saved_lists::short_datetime;

#[derive(Debug, Clone)]
pub enum BackgroundMsg {
    /// La revisión `request` de `masters.json`: `None` = no tocaba; si no, lo de
    /// `check_official` (la lista si cambió, y si se pudo bajar).
    Masters {
        request: u64,
        force: bool,
        result: Option<(Option<OfficialList>, bool)>,
    },
}

/// Un pedido de fondo a la vez; el número no vuelve a empezar, así lo de uno viejo se descarta.
#[derive(Default)]
pub(crate) struct Request {
    serial: u64,
    busy: bool,
}

impl Request {
    fn start(&mut self) -> u64 {
        self.serial += 1;
        self.busy = true;
        self.serial
    }

    /// ¿Es la respuesta del pedido en curso? Si lo es, termina.
    fn finish(&mut self, request: u64) -> bool {
        let current = self.busy && request == self.serial;
        if current {
            self.busy = false;
        }
        current
    }
}

impl App {
    pub fn official(&self) -> &OfficialList {
        &self.official
    }

    pub fn masters_busy(&self) -> bool {
        self.masters_request.busy
    }

    /// El número del último pedido de masters.
    pub fn masters_request(&self) -> u64 {
        self.masters_request.serial
    }

    /// `check_masters_update`: al arrancar (`force = false`) y con *Buscar actualización*.
    pub fn check_masters_update(&mut self, force: bool) -> Task<Message> {
        let Some(fetch) = self.deps.masters_fetch.clone() else {
            // `_force_masters_check` sin red: la pestaña lo dice enseguida.
            if force {
                let today = self.today();
                if let Some(form) = &mut self.settings {
                    form.masters
                        .show_update_result(Outcome::Failed, None, &today);
                }
            }
            return Task::none();
        };
        if self.masters_busy() {
            return Task::none(); // el que corre informará
        }
        let request = self.masters_request.start();
        let cache = self
            .cfg_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_default()
            .join("cache");
        let now = (self.deps.now)().0;
        let work = move || {
            (force || should_check(&cache.join(META_FILENAME), now))
                .then(|| check_official(&cache, now, |url| fetch(url)))
        };
        Task::perform(blocking(work), move |r| {
            Message::Background(BackgroundMsg::Masters {
                request,
                force,
                result: r.unwrap_or(Some((None, false))),
            })
        })
    }

    pub(super) fn on_background(&mut self, m: BackgroundMsg) -> Task<Message> {
        let BackgroundMsg::Masters {
            request,
            force,
            result,
        } = m;
        if !self.masters_request.finish(request) {
            return Task::none(); // reemplazado o de otra sesión
        }
        let Some((list, ok)) = result else {
            return Task::none(); // no tocaba
        };
        let kind = match (&list, ok) {
            (Some(_), _) => Outcome::Updated,
            (None, true) => Outcome::Unchanged,
            (None, false) => Outcome::Failed,
        };
        let today = self.today();
        let date = list.as_ref().map(|l| match l.updated.as_str() {
            "" => today.clone(),
            d => d.to_owned(),
        });
        if let Some(l) = &list {
            self.official = l.clone();
        }
        // Una revisión automática que falló o no trajo nada no pisa la pestaña abierta.
        let open = self.settings.is_some();
        if let Some(form) = self
            .settings
            .as_mut()
            .filter(|_| kind == Outcome::Updated || force)
        {
            form.masters.show_update_result(kind, list, &today);
        }
        let text = match (kind, date) {
            (Outcome::Updated, Some(date)) => {
                tr_fmt("Lista de masters actualizada ({date})", &[("date", &date)])
            }
            _ if !force || open => return Task::none(),
            (Outcome::Unchanged, _) => tr("La lista de masters está al día"),
            _ => tr("No se pudo descargar la lista de masters"),
        };
        self.show_message(text, Some(Duration::from_secs(5)))
    }

    /// `time.strftime("%Y-%m-%d")` con el reloj de `Deps`.
    fn today(&self) -> String {
        let (now, offset) = (self.deps.now)();
        let stamp = short_datetime(now, offset);
        stamp.split(' ').next().unwrap_or_default().to_owned()
    }
}
