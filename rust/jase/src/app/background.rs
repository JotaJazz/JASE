//! Tareas de fondo (`window_background.py`): la lista oficial de masters (como mucho cada
//! 24 h salvo a mano desde Ajustes) y la versión nueva de JASE, con su descarga verificada y
//! el reemplazo. Leer el `.meta.json`, bajar, extraer y lanzar procesos van en un hilo.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures::SinkExt;
use iced::widget::{Space, column, row, text};
use iced::{Element, Fill, Subscription, Task, time};
use jase_core::app_update::{self, RELEASE_PAGE_PREFIX, Release};
use jase_core::i18n::{tr, tr_fmt};
use jase_core::masters::{META_FILENAME, OfficialList, check_official, should_check};
use jase_core::portable_update::write_updater;

use super::dialogs::card;
use super::{App, Dialog, Message, blocking};
use crate::masters_tab::Outcome;
use crate::saved_lists::short_datetime;
use crate::theme::{style, widget_style};
use crate::update::{SelfUpdate, download_error_text, download_update};
use crate::{bars, widgets, window_state};

/// Cada cuánto se pregunta por una versión nueva; el plazo real de 24 h lo pone `should_check`.
pub const UPDATE_POLL: Duration = Duration::from_secs(3600);

#[derive(Debug, Clone)]
pub enum BackgroundMsg {
    /// La revisión `request` de `masters.json`: `None` = no tocaba; si no, lo de
    /// `check_official` (la lista si cambió, y si se pudo bajar).
    Masters {
        request: u64,
        force: bool,
        result: Option<(Option<OfficialList>, bool)>,
    },
    /// Venció `UPDATE_POLL`: la consulta automática de la versión.
    CheckUpdates,
    /// La consulta `request`: `None` = no tocaba; si no, lo de `check_detailed`.
    Update {
        request: u64,
        manual: bool,
        result: Option<(Option<Release>, bool)>,
    },
    /// Clic en el enlace de la barra de estado.
    UpdateLink,
    UpdateChoice(UpdateChoice),
    /// Cambió el porcentaje de la descarga `request`.
    Progress {
        request: u64,
        percent: u8,
    },
    /// Terminó la descarga `request`: el archivo (o la carpeta extraída) o el código de error.
    Downloaded {
        request: u64,
        result: Result<PathBuf, String>,
    },
    /// *Cancelar* (o Esc) en el diálogo de la descarga.
    CancelDownload,
    /// Se lanzó (o no) el instalador o el script de la descarga `request`.
    Launched {
        request: u64,
        ok: bool,
    },
    /// *Ver la página* del aviso de una actualización fallida.
    OpenPage,
}

/// Los botones de «Nueva versión disponible».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateChoice {
    Install,
    Page,
    Cancel,
}

/// La descarga en curso; sigue hasta que se lanza lo descargado.
pub(crate) struct Download {
    request: u64,
    cancel: Arc<AtomicBool>,
    handle: iced::task::Handle,
    /// La carpeta del portátil que se reemplaza; `None` = instalador.
    exe_dir: Option<PathBuf>,
    dest: PathBuf,
}

/// La versión nueva de JASE: la consulta, la release avisada y la descarga.
#[derive(Default)]
pub(crate) struct Updates {
    request: Request,
    release: Option<Release>,
    download: Option<Download>,
    downloads: u64,
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
        let cache = self.cache_dir();
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

    fn cache_dir(&self) -> PathBuf {
        let dir = self.cfg_path.parent().map(PathBuf::from);
        dir.unwrap_or_default().join("cache")
    }

    /// Al arrancar: la lista de masters y la versión nueva (las dos, como mucho cada 24 h).
    pub(super) fn start_background(&mut self) -> Task<Message> {
        Task::batch([
            self.check_masters_update(false),
            self.check_app_update(false),
        ])
    }

    /// La consulta automática cada hora, solo con red.
    pub(super) fn update_subscription(&self) -> Option<Subscription<Message>> {
        self.deps.update_fetch.is_some().then(|| {
            time::every(UPDATE_POLL).map(|_| Message::Background(BackgroundMsg::CheckUpdates))
        })
    }

    pub(super) fn on_background(&mut self, m: BackgroundMsg) -> Task<Message> {
        use BackgroundMsg as B;
        match m {
            B::Masters {
                request,
                force,
                result,
            } => self.on_masters_checked(request, force, result),
            B::CheckUpdates => self.check_app_update(false),
            B::Update {
                request,
                manual,
                result,
            } => self.on_update(request, manual, result),
            B::UpdateLink => self.on_update_link(),
            B::UpdateChoice(choice) if matches!(self.dialog(), Some(Dialog::UpdateAsk { .. })) => {
                let close = self.close_dialog();
                let task = match choice {
                    UpdateChoice::Install => self.start_download(),
                    UpdateChoice::Page => self.open_release_page(),
                    UpdateChoice::Cancel => Task::none(),
                };
                Task::batch([close, task])
            }
            B::Progress { request, percent } if self.download_is(request) => {
                if let Some((Dialog::Download { percent: p, .. }, _)) = &mut self.dialog {
                    *p = percent;
                }
                Task::none()
            }
            B::Downloaded { request, result } if self.download_is(request) => {
                self.on_downloaded(result)
            }
            B::CancelDownload if matches!(self.dialog(), Some(Dialog::Download { .. })) => {
                self.stop_download();
                self.close_dialog()
            }
            B::Launched { request, ok } if self.download_is(request) => self.on_update_launched(ok),
            B::OpenPage if matches!(self.dialog(), Some(Dialog::UpdateFailed { .. })) => {
                let close = self.close_dialog();
                Task::batch([close, self.open_release_page()])
            }
            // De una descarga cancelada o reemplazada, o sin su diálogo.
            _ => Task::none(),
        }
    }

    fn on_masters_checked(
        &mut self,
        request: u64,
        force: bool,
        result: Option<(Option<OfficialList>, bool)>,
    ) -> Task<Message> {
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

impl App {
    pub fn update_release(&self) -> Option<&Release> {
        self.updates.release.as_ref()
    }

    /// El enlace de la barra de estado, si hay versión nueva.
    pub fn update_link_text(&self) -> Option<String> {
        let release = self.updates.release.as_ref()?;
        Some(tr_fmt(
            "Nueva versión {version} disponible",
            &[("version", &release.version)],
        ))
    }

    pub fn downloading(&self) -> bool {
        self.updates.download.is_some()
    }

    /// La bandera que corta el hilo de la descarga en curso.
    pub fn download_cancel_flag(&self) -> Option<Arc<AtomicBool>> {
        self.updates.download.as_ref().map(|d| d.cancel.clone())
    }

    pub fn update_busy(&self) -> bool {
        self.updates.request.busy
    }

    /// El número de la última consulta de versión.
    pub fn update_request(&self) -> u64 {
        self.updates.request.serial
    }

    fn download_is(&self, request: u64) -> bool {
        self.updates
            .download
            .as_ref()
            .is_some_and(|d| d.request == request)
    }

    /// `check_app_update` (automática: al arrancar y cada hora, si está activada y toca) o
    /// `check_app_update_now` (manual: aunque esté apagada, sin esperar las 24 h, y reemplaza
    /// a la automática en curso).
    pub fn check_app_update(&mut self, manual: bool) -> Task<Message> {
        let Some(fetch) = self.deps.update_fetch.clone() else {
            return Task::none();
        };
        if !manual && (!self.cfg.check_app_updates || self.update_busy()) {
            return Task::none();
        }
        let request = self.updates.request.start();
        if manual {
            self.set_status(tr("Buscando actualizaciones…"));
        }
        let (cache, now, version) = (self.cache_dir(), (self.deps.now)().0, self.deps.version);
        let work = move || {
            let meta = cache.join(app_update::META_FILENAME);
            (manual || app_update::should_check(&meta, now))
                .then(|| app_update::check_detailed(&cache, now, version, |url| fetch(url)))
        };
        Task::perform(blocking(work), move |r| {
            Message::Background(BackgroundMsg::Update {
                request,
                manual,
                result: r.unwrap_or(Some((None, false))),
            })
        })
    }

    /// `_on_update_checked`: solo una release de la página de JASE vale.
    fn on_update(
        &mut self,
        request: u64,
        manual: bool,
        result: Option<(Option<Release>, bool)>,
    ) -> Task<Message> {
        if !self.updates.request.finish(request) {
            return Task::none(); // reemplazada por la manual
        }
        let Some((release, ok)) = result else {
            return Task::none(); // no tocaba
        };
        let valid = release.filter(|r| r.url.starts_with(RELEASE_PAGE_PREFIX));
        if manual {
            self.set_status(String::new());
            if valid.is_none() {
                let text = if ok {
                    let version = self.deps.version;
                    tr_fmt(
                        "Tienes la última versión ({version}).",
                        &[("version", &version)],
                    )
                } else {
                    tr("No se pudo consultar GitHub. Revisa la conexión e inténtalo más tarde.")
                };
                self.notice(tr("Buscar actualizaciones"), text);
                return Task::none();
            }
        }
        let Some(release) = valid else {
            return Task::none();
        };
        self.updates.release = Some(release);
        if manual {
            return self.on_update_link(); // se pidió a mano: se ofrece ya
        }
        Task::none()
    }

    /// Un aviso; con otro diálogo abierto no lo pisa: va a la barra de estado.
    fn notice(&mut self, title: String, text: String) {
        if self.dialog.is_some() {
            self.set_status(format!("{title}: {text}"));
        } else {
            self.open_dialog(Dialog::Message {
                title,
                text,
                informative: String::new(),
                settings: false,
            });
        }
    }

    /// El asset que sirve para reemplazar este ejecutable y, si es el portátil, su carpeta.
    fn update_asset(&self) -> Option<(app_update::Asset, Option<PathBuf>)> {
        let release = self.updates.release.as_ref()?;
        match &self.deps.self_update {
            SelfUpdate::Unsupported => None,
            SelfUpdate::Installer => Some((release.installer.clone()?, None)),
            SelfUpdate::Portable(dir) => Some((release.portable.clone()?, Some(dir.clone()))),
        }
    }

    /// `_on_update_link`: en Windows con su asset, la pregunta; si no, la página.
    fn on_update_link(&mut self) -> Task<Message> {
        let Some(release) = &self.updates.release else {
            return Task::none();
        };
        if self.update_asset().is_none() {
            return self.open_release_page();
        }
        // Debajo de otro diálogo o de la descarga no: el enlace sigue en la barra.
        if self.dialog.is_some() || self.downloading() {
            return Task::none();
        }
        let version = release.version.clone();
        self.open_dialog(Dialog::UpdateAsk { version });
        Task::none()
    }

    fn open_release_page(&self) -> Task<Message> {
        let Some(release) = &self.updates.release else {
            return Task::none();
        };
        let (open, url) = (self.deps.open_url.clone(), release.url.clone());
        Task::perform(blocking(move || open(&url)), |_| ()).discard()
    }

    /// `_start_update_download`: la descarga en un hilo; el progreso llega solo cuando cambia
    /// el porcentaje (como mucho 101 mensajes) y al final el resultado.
    fn start_download(&mut self) -> Task<Message> {
        let Some((asset, exe_dir)) = self.update_asset() else {
            return Task::none();
        };
        if self.downloading() {
            return Task::none();
        }
        let dest = self.cache_dir().join("updates");
        let extract_to = exe_dir
            .is_some()
            .then(|| dest.join(format!("portable-{}", asset.name)));
        self.updates.downloads += 1;
        let request = self.updates.downloads;
        let cancel = Arc::new(AtomicBool::new(false));
        let (stream, flag, to) = (
            self.deps.update_stream.clone(),
            cancel.clone(),
            dest.clone(),
        );
        let name = asset.name.clone();
        let messages = iced::stream::channel(16, async move |mut tx| {
            let mut progress_tx = tx.clone();
            let work = move || {
                let mut last = None;
                let mut progress = |done: u64, total: u64| {
                    let percent = (done.saturating_mul(100) / total.max(1)).min(100) as u8;
                    if last != Some(percent) {
                        last = Some(percent);
                        let msg = BackgroundMsg::Progress { request, percent };
                        // Con la `Task` abortada el canal está cerrado y esto no espera.
                        let _ = futures::executor::block_on(progress_tx.send(msg));
                    }
                };
                let cancelled = || flag.load(Ordering::Relaxed);
                download_update(
                    &asset,
                    &to,
                    extract_to.as_deref(),
                    &stream,
                    &mut progress,
                    &cancelled,
                )
            };
            let result = blocking(work).await.and_then(|r| r);
            let _ = tx.send(BackgroundMsg::Downloaded { request, result }).await;
        });
        let (task, handle) = Task::run(messages, Message::Background).abortable();
        self.updates.download = Some(Download {
            request,
            cancel,
            handle,
            exe_dir,
            dest,
        });
        self.open_dialog(Dialog::Download { name, percent: 0 });
        task
    }

    /// *Cancelar*, el diálogo reemplazado o el cierre: la UI no espera al hilo (la bandera lo
    /// corta en el próximo trozo) y lo que llegue de él se descarta.
    pub(super) fn stop_download(&mut self) {
        if let Some(d) = self.updates.download.take() {
            d.cancel.store(true, Ordering::Relaxed);
            d.handle.abort();
        }
    }

    /// `_on_update_downloaded`: verificado (y extraído), se lanza en un hilo; si no, el aviso.
    fn on_downloaded(&mut self, result: Result<PathBuf, String>) -> Task<Message> {
        let Some(d) = &self.updates.download else {
            return Task::none();
        };
        let path = match result {
            Ok(path) => path,
            Err(code) => {
                self.updates.download = None;
                let error = download_error_text(&code);
                let text = tr_fmt(
                    "No se pudo descargar la actualización: {error}",
                    &[("error", &error)],
                );
                self.open_dialog(Dialog::UpdateFailed { text });
                return Task::none();
            }
        };
        let (request, spawn) = (d.request, self.deps.spawn.clone());
        let (exe_dir, dest) = (d.exe_dir.clone(), d.dest.clone());
        let work = move || match exe_dir {
            // `_start_portable_swap`: el script reemplaza la carpeta cuando JASE se cierre.
            Some(exe_dir) => {
                write_updater(&dest, std::process::id(), &path, &exe_dir).is_ok_and(|script| {
                    let args = ["/c".to_owned(), script.to_string_lossy().into_owned()];
                    spawn(Path::new("cmd.exe"), &args).is_ok()
                })
            }
            None => spawn(&path, &[]).is_ok(),
        };
        let launch = Task::perform(blocking(work), move |ok| {
            Message::Background(BackgroundMsg::Launched {
                request,
                ok: ok.unwrap_or(false),
            })
        });
        Task::batch([self.close_dialog(), launch])
    }

    /// Lanzado: se guarda y sale (`_quit_app`); si no, el aviso y la app sigue.
    fn on_update_launched(&mut self, ok: bool) -> Task<Message> {
        let Some(d) = self.updates.download.take() else {
            return Task::none();
        };
        if ok {
            return window_state::close();
        }
        let text = if d.exe_dir.is_some() {
            tr("No se pudo reemplazar la carpeta de JASE; actualízala a mano.")
        } else {
            tr("No se pudo ejecutar el instalador.")
        };
        self.open_dialog(Dialog::UpdateFailed { text });
        Task::none()
    }

    /// `ask_action`: *Descargar e instalar*, *Ver la página* o *Cancelar*.
    pub(super) fn update_ask_view(&self, version: &str) -> Element<'static, Message> {
        let msg = |c| Message::Background(BackgroundMsg::UpdateChoice(c));
        let version = [("version", &version as &dyn std::fmt::Display)];
        self.update_card(
            tr_fmt("Nueva versión {version} disponible", &version),
            tr_fmt("Hay una versión nueva de JASE ({version}).", &version),
            None,
            vec![
                (tr("Descargar e instalar"), msg(UpdateChoice::Install)),
                (tr("Ver la página"), msg(UpdateChoice::Page)),
                (tr("Cancelar"), msg(UpdateChoice::Cancel)),
            ],
        )
    }

    /// El `QProgressDialog` de la descarga.
    pub(super) fn download_view(&self, name: &str, percent: u8) -> Element<'static, Message> {
        let bar = bars::progress_bar(
            self.theme_id,
            usize::from(percent),
            100,
            format!("{percent}%"),
            300.0,
        );
        self.update_card(
            tr("Actualizar JASE"),
            tr_fmt("Descargando {name}…", &[("name", &name)]),
            Some(bar),
            vec![(
                tr("Cancelar"),
                Message::Background(BackgroundMsg::CancelDownload),
            )],
        )
    }

    /// El aviso de una actualización que falló, con *Ver la página*.
    pub(super) fn update_failed_view(&self, body: &str) -> Element<'static, Message> {
        self.update_card(
            tr("Actualizar JASE"),
            body.to_owned(),
            None,
            vec![
                (
                    tr("Ver la página"),
                    Message::Background(BackgroundMsg::OpenPage),
                ),
                (tr("Cerrar"), Message::CloseDialog),
            ],
        )
    }

    fn update_card(
        &self,
        title: String,
        body: String,
        extra: Option<Element<'static, Message>>,
        buttons: Vec<(String, Message)>,
    ) -> Element<'static, Message> {
        let theme_id = self.theme_id;
        let s = widget_style(theme_id);
        let label = |t: String| text(t).font(s.font()).size(s.text_size());
        let buttons = buttons
            .into_iter()
            .map(|(t, m)| widgets::button(t, m, theme_id));
        let buttons = row![Space::new().width(Fill)].extend(buttons).spacing(6);
        let content = column![label(title).font(style::header_font(s)), label(body)]
            .push(extra)
            .push(buttons);
        card(content, 360.0, theme_id)
    }
}
