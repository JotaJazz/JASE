//! La bandeja en Linux: el icono por StatusNotifierItem (`ksni`) y los globos por
//! `org.freedesktop.Notifications` (`zbus`), como `QSystemTrayIcon` en KDE.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use futures::channel::mpsc::{UnboundedSender, unbounded};
use futures::stream::{self, BoxStream};
use futures::{SinkExt, StreamExt};
use ksni::TrayMethods;
use ksni::menu::StandardItem;
use zbus::zvariant::Value;

use super::{TrayCommand, TrayEvent, TrayHandle, TrayLabels, escape_markup, plain_notice};
use super::{rgba_to_argb, until_dropped};
use crate::icons;
use crate::startup::APP_NAME;

const NOTIFICATIONS: &str = "org.freedesktop.Notifications";

struct JaseTray {
    labels: TrayLabels,
    icons: Vec<ksni::Icon>,
    events: UnboundedSender<TrayEvent>,
}

impl JaseTray {
    fn send(&self, e: TrayEvent) {
        let _ = self.events.unbounded_send(e);
    }
}

impl ksni::Tray for JaseTray {
    fn id(&self) -> String {
        "jase".into()
    }

    fn title(&self) -> String {
        APP_NAME.into()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: APP_NAME.into(),
            ..Default::default()
        }
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.icons.clone()
    }

    // En KDE el clic simple es la activación (Qt restaura con doble clic, que SNI no manda).
    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayEvent::Show);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        let item = |label: &str, e: fn() -> TrayEvent| {
            StandardItem {
                label: label.replace('_', "__"),
                activate: Box::new(move |t: &mut Self| t.send(e())),
                ..Default::default()
            }
            .into()
        };
        vec![
            item(&self.labels.show, || TrayEvent::Show),
            item(&self.labels.quit, || TrayEvent::Quit),
        ]
    }
}

fn pixmaps() -> Vec<ksni::Icon> {
    [32, 64]
        .into_iter()
        .filter_map(|side| {
            let rgba = icons::app_icon_rgba(side)?;
            let side = side as i32;
            Some(ksni::Icon {
                width: side,
                height: side,
                data: rgba_to_argb(&rgba),
            })
        })
        .collect()
}

/// El proxy de notificaciones, si el servicio responde (`supportsMessages`).
async fn notifications() -> Option<zbus::Proxy<'static>> {
    let conn = zbus::Connection::session().await.ok()?;
    let proxy = zbus::Proxy::new_owned(
        conn,
        NOTIFICATIONS,
        "/org/freedesktop/Notifications",
        NOTIFICATIONS,
    )
    .await
    .ok()?;
    let _: Vec<String> = proxy.call("GetCapabilities", &()).await.ok()?;
    Some(proxy)
}

enum Input {
    Tray(TrayEvent),
    Command(TrayCommand),
    /// `ActionInvoked` con la id de la notificación.
    Clicked(u32),
}

pub fn run(labels: &TrayLabels) -> BoxStream<'static, TrayEvent> {
    let labels = labels.clone();
    let service: Arc<OnceLock<ksni::Handle<JaseTray>>> = Arc::default();
    let service2 = service.clone();
    let close: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        if let Some(h) = service2.get() {
            // Encola el cierre: no hace falta esperarlo.
            drop(h.shutdown());
        }
    });
    let close2 = close.clone();
    let events = iced::stream::channel(16, async move |mut out| {
        let (tx, tray_events) = unbounded();
        let tray = JaseTray {
            labels,
            icons: pixmaps(),
            events: tx,
        };
        // Sin D-Bus o sin `StatusNotifierWatcher` (WSL) falla enseguida: no hay bandeja.
        let Ok(handle) = tray.spawn().await else {
            return;
        };
        let _ = service.set(handle);
        let proxy = notifications().await;
        let clicks = match &proxy {
            Some(p) => p.receive_signal("ActionInvoked").await.ok(),
            None => None,
        };
        let clicks = match clicks {
            Some(s) => s
                .filter_map(|m| async move {
                    let (id, action): (u32, String) = m.body().deserialize().ok()?;
                    (action == "default").then_some(Input::Clicked(id))
                })
                .boxed(),
            None => stream::pending().boxed(),
        };
        let (app_handle, commands) = TrayHandle::new(proxy.is_some(), close2);
        if out.send(TrayEvent::Ready(app_handle)).await.is_err() {
            return;
        }
        let mut inputs = stream::select_all([
            tray_events.map(Input::Tray).boxed(),
            commands.map(Input::Command).boxed(),
            clicks,
        ]);
        let mut last: Option<u32> = None;
        while let Some(input) = inputs.next().await {
            match input {
                Input::Tray(e) => {
                    if out.send(e).await.is_err() {
                        return;
                    }
                }
                Input::Command(TrayCommand::Notify { title, text }) => {
                    let Some(p) = &proxy else { continue };
                    let body = (
                        APP_NAME,
                        0u32,
                        "",
                        plain_notice(&title),
                        escape_markup(&plain_notice(&text)),
                        vec!["default", ""],
                        HashMap::<&str, Value>::new(),
                        -1i32,
                    );
                    last = p.call("Notify", &body).await.ok();
                }
                Input::Clicked(id) => {
                    if last == Some(id) && out.send(TrayEvent::MessageClicked).await.is_err() {
                        return;
                    }
                }
            }
        }
    });
    until_dropped(events.boxed(), close)
}
