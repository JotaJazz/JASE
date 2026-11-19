//! La bandeja en Windows: `Shell_NotifyIconW` a mano. Un hilo propio con una ventana oculta
//! (nunca se muestra) recibe los clics del icono en su bucle de mensajes; los globos y el
//! cierre se piden desde fuera con la `HWND` (el shell no exige el hilo dueño).

use std::cell::RefCell;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::{mem, ptr, thread};

use futures::channel::mpsc::{UnboundedSender, unbounded};
use futures::channel::oneshot;
use futures::stream::BoxStream;
use futures::{SinkExt, StreamExt};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{CreateBitmap, DeleteObject};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NIM_SETVERSION, NIN_BALLOONUSERCLICK, NOTIFYICON_VERSION_4, NOTIFYICONDATAW, NOTIFYICONDATAW_0,
    Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon,
    DestroyMenu, DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW, HICON, ICONINFO,
    IDI_APPLICATION, LoadIconW, MF_STRING, MSG, PostMessageW, PostQuitMessage, RegisterClassW,
    RegisterWindowMessageW, SetForegroundWindow, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    TrackPopupMenu, TranslateMessage, WM_APP, WM_CLOSE, WM_CONTEXTMENU, WM_DESTROY,
    WM_LBUTTONDBLCLK, WM_NULL, WNDCLASSW,
};

use super::{TrayCommand, TrayEvent, TrayHandle, TrayLabels};
use super::{plain_notice, until_dropped, wide_truncated};
use crate::icons;
use crate::startup::APP_NAME;

/// Los clics del icono llegan como este mensaje (versión 4: el evento en `LOWORD(lParam)`).
const CALLBACK: u32 = WM_APP + 1;
const ICON_ID: u32 = 1;
const ID_SHOW: usize = 1;
const ID_QUIT: usize = 2;
/// `TaskbarCreated`: el explorador se reinició y hay que volver a agregar el icono.
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

/// Lo que el hilo de la ventana necesita en su `wndproc`.
struct Ctx {
    events: UnboundedSender<TrayEvent>,
    show: Vec<u16>,
    quit: Vec<u16>,
    icon: HICON,
}

thread_local! {
    static CTX: RefCell<Option<Ctx>> = const { RefCell::new(None) };
}

/// La ventana del hilo, compartida con el cierre: `closed` antes de tenerla = no crear nada.
#[derive(Default)]
struct Shared {
    hwnd: usize,
    closed: bool,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

/// `s` en un campo fijo de `NOTIFYICONDATAW`, cortado sin partir un par sustituto.
fn fill<const N: usize>(dst: &mut [u16; N], s: &str) {
    let w = wide_truncated(s, N - 1);
    dst[..w.len()].copy_from_slice(&w);
}

fn icon_data(hwnd: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: ICON_ID,
        ..Default::default()
    }
}

fn add_icon(hwnd: HWND, icon: HICON) {
    let mut nid = icon_data(hwnd);
    nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    nid.uCallbackMessage = CALLBACK;
    nid.hIcon = icon;
    fill(&mut nid.szTip, APP_NAME);
    nid.Anonymous = NOTIFYICONDATAW_0 {
        uVersion: NOTIFYICON_VERSION_4,
    };
    // SAFETY: `nid` es un `NOTIFYICONDATAW` completo con su `cbSize` y vive durante la llamada.
    unsafe {
        Shell_NotifyIconW(NIM_ADD, &nid);
        Shell_NotifyIconW(NIM_SETVERSION, &nid);
    }
}

fn delete_icon(hwnd: HWND) {
    let nid = icon_data(hwnd);
    // SAFETY: como en `add_icon`; quitar un icono que ya no está solo devuelve FALSE.
    unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
}

fn balloon(hwnd: HWND, title: &str, text: &str) {
    let mut nid = icon_data(hwnd);
    nid.uFlags = NIF_INFO;
    nid.dwInfoFlags = NIIF_INFO;
    fill(&mut nid.szInfoTitle, &plain_notice(title));
    fill(&mut nid.szInfo, &plain_notice(text));
    // SAFETY: como en `add_icon`; con la ventana ya cerrada la llamada solo falla.
    unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid) };
}

/// `app_icon.svg` a 32 px como `HICON` (`true` = nuestro, hay que destruirlo); si falla, el
/// icono genérico del sistema.
fn app_hicon() -> (HICON, bool) {
    const SIDE: i32 = 32;
    let made = icons::app_icon_rgba(SIDE as u32).and_then(|rgba| {
        let (pixels, _) = rgba.as_chunks::<4>();
        let bgra: Vec<u8> = pixels
            .iter()
            .flat_map(|&[r, g, b, a]| [b, g, r, a])
            .collect();
        // Máscara AND de 1 bit en cero (filas de 4 bytes): manda el alfa del color.
        let mask = vec![0u8; (SIDE * SIDE / 8) as usize];
        // SAFETY: `bgra` tiene SIDE × SIDE píxeles de 32 bits y `mask` SIDE filas de 1 bit
        // alineadas a 16 bits; los bitmaps se copian en el icono y se borran enseguida.
        unsafe {
            let color = CreateBitmap(SIDE, SIDE, 1, 32, bgra.as_ptr().cast());
            let and = CreateBitmap(SIDE, SIDE, 1, 1, mask.as_ptr().cast());
            let info = ICONINFO {
                fIcon: 1,
                xHotspot: 0,
                yHotspot: 0,
                hbmMask: and,
                hbmColor: color,
            };
            let icon = if color.is_null() || and.is_null() {
                ptr::null_mut()
            } else {
                CreateIconIndirect(&info)
            };
            DeleteObject(color);
            DeleteObject(and);
            (!icon.is_null()).then_some(icon)
        }
    });
    match made {
        Some(icon) => (icon, true),
        // SAFETY: icono de sistema compartido (instancia nula); no se destruye.
        None => (
            unsafe { LoadIconW(ptr::null_mut(), IDI_APPLICATION) },
            false,
        ),
    }
}

fn send(e: TrayEvent) {
    CTX.with_borrow(|c| {
        if let Some(c) = c {
            let _ = c.events.unbounded_send(e);
        }
    });
}

/// Clic derecho: el menú *Mostrar JASE* / *Salir* en el cursor.
fn context_menu(hwnd: HWND) {
    // Copia de los textos: `TrackPopupMenu` bombea mensajes y vuelve a entrar en `wndproc`.
    let Some((show, quit)) =
        CTX.with_borrow(|c| c.as_ref().map(|c| (c.show.clone(), c.quit.clone())))
    else {
        return;
    };
    // SAFETY: `hwnd` es la ventana de este hilo; el menú es nuestro y se destruye al final;
    // los textos viven hasta después de `TrackPopupMenu`.
    let cmd = unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        AppendMenuW(menu, MF_STRING, ID_SHOW, show.as_ptr());
        AppendMenuW(menu, MF_STRING, ID_QUIT, quit.as_ptr());
        let mut pt = POINT::default();
        GetCursorPos(&mut pt);
        // Sin esto el menú no se cierra al hacer clic fuera (documentado en `TrackPopupMenu`).
        SetForegroundWindow(hwnd);
        let flags = TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY;
        let cmd = TrackPopupMenu(menu, flags, pt.x, pt.y, 0, hwnd, ptr::null());
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);
        cmd
    };
    match cmd as usize {
        ID_SHOW => send(TrayEvent::Show),
        ID_QUIT => send(TrayEvent::Quit),
        _ => {}
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        CALLBACK => {
            match (lparam as u32) & 0xFFFF {
                WM_LBUTTONDBLCLK => send(TrayEvent::Show),
                WM_CONTEXTMENU => context_menu(hwnd),
                NIN_BALLOONUSERCLICK => send(TrayEvent::MessageClicked),
                _ => {}
            }
            return 0;
        }
        WM_CLOSE => {
            // SAFETY: la ventana es de este hilo.
            unsafe { DestroyWindow(hwnd) };
            return 0;
        }
        WM_DESTROY => {
            delete_icon(hwnd);
            // SAFETY: sale del bucle de mensajes de este hilo.
            unsafe { PostQuitMessage(0) };
            return 0;
        }
        m if m != 0 && m == TASKBAR_CREATED.load(Ordering::Relaxed) => {
            if let Some(icon) = CTX.with_borrow(|c| c.as_ref().map(|c| c.icon)) {
                add_icon(hwnd, icon);
            }
        }
        _ => {}
    }
    // SAFETY: los mismos argumentos que recibió `wndproc`.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// El hilo del icono: crea la ventana oculta y el icono, avisa la `HWND` y bombea mensajes
/// hasta `WM_CLOSE`. Si algo falla, suelta `ready` sin mandar: el stream termina (sin bandeja).
fn tray_thread(
    labels: TrayLabels,
    events: UnboundedSender<TrayEvent>,
    shared: Arc<Mutex<Shared>>,
    ready: oneshot::Sender<usize>,
) {
    let class = wide("JaseTray");
    // SAFETY: `class` vive todo el hilo; registrar la clase de nuevo (otro icono tras cambiar
    // de idioma) falla sin daño; la ventana es de este hilo y nunca se muestra.
    let hwnd = unsafe {
        TASKBAR_CREATED.store(
            RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
            Ordering::Relaxed,
        );
        let instance = GetModuleHandleW(ptr::null());
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            ..Default::default()
        };
        RegisterClassW(&wc);
        CreateWindowExW(
            0,
            class.as_ptr(),
            class.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null(),
        )
    };
    if hwnd.is_null() {
        return;
    }
    let (icon, owned) = app_hicon();
    CTX.set(Some(Ctx {
        events,
        show: wide(&labels.show),
        quit: wide(&labels.quit),
        icon,
    }));
    add_icon(hwnd, icon);
    let closed = {
        let mut s = shared.lock().unwrap_or_else(PoisonError::into_inner);
        s.hwnd = hwnd as usize;
        s.closed
    };
    if closed {
        // Soltado mientras nacía: nada queda.
        // SAFETY: la ventana es de este hilo.
        unsafe { DestroyWindow(hwnd) };
    } else {
        let _ = ready.send(hwnd as usize);
    }
    let mut msg = MSG::default();
    // SAFETY: bucle de mensajes clásico sobre un `MSG` propio; termina con `PostQuitMessage`.
    unsafe {
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    CTX.take();
    if owned {
        // SAFETY: el icono es nuestro (`CreateIconIndirect`) y ya no lo usa nadie.
        unsafe { DestroyIcon(icon) };
    }
}

pub fn run(labels: &TrayLabels) -> BoxStream<'static, TrayEvent> {
    let labels = labels.clone();
    let shared = Arc::new(Mutex::new(Shared::default()));
    let (events_tx, mut events) = unbounded();
    let (ready_tx, ready) = oneshot::channel();
    let thread_shared = shared.clone();
    let spawned = thread::Builder::new()
        .name("jase-tray".into())
        .spawn(move || tray_thread(labels, events_tx, thread_shared, ready_tx));
    // Quitar el icono en el acto (sin fantasma al salir con `process::exit`) y cerrar la ventana.
    let close: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        let mut s = shared.lock().unwrap_or_else(PoisonError::into_inner);
        s.closed = true;
        if s.hwnd != 0 {
            let hwnd = s.hwnd as HWND;
            delete_icon(hwnd);
            // SAFETY: `hwnd` es la ventana del hilo del icono, todavía viva (se pone en 0 aquí,
            // una sola vez); `PostMessageW` sirve desde cualquier hilo.
            unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
            s.hwnd = 0;
        }
    });
    let close2 = close.clone();
    let stream = iced::stream::channel(16, async move |mut out| {
        if spawned.is_err() {
            return;
        }
        let Ok(hwnd) = ready.await else {
            return;
        };
        let (handle, mut commands) = TrayHandle::new(true, close2);
        if out.send(TrayEvent::Ready(handle)).await.is_err() {
            return;
        }
        loop {
            futures::select! {
                e = events.next() => match e {
                    Some(e) => {
                        if out.send(e).await.is_err() {
                            return;
                        }
                    }
                    None => return,
                },
                c = commands.next() => {
                    if let Some(TrayCommand::Notify { title, text }) = c {
                        balloon(hwnd as HWND, &title, &text);
                    }
                }
                complete => return,
            }
        }
    });
    until_dropped(stream.boxed(), close)
}
