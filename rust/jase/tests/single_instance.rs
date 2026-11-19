//! Instancia única (`tests/test_single_instance.py`): el socket de la primera JASE recibe
//! `show` o el enlace de la segunda. Runtime de tokio normal: con el reloj en pausa el tiempo
//! salta en cuanto el runtime queda ocioso, aunque haya datos en camino por otro hilo.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use futures::StreamExt;
use futures::channel::mpsc;
use jase::single_instance::{
    MAX_BYTES, SEND_TIMEOUT, read_line, send_to_running, serve, server_name,
};
use tokio::io::AsyncWriteExt;
use tokio::runtime::Runtime;

/// El punto de encuentro de un test: un archivo en su carpeta o un *pipe* propio.
#[cfg(not(windows))]
fn test_endpoint(dir: &Path) -> PathBuf {
    dir.join("s")
}

#[cfg(windows)]
fn test_endpoint(_dir: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let n = N.fetch_add(1, Ordering::SeqCst);
    PathBuf::from(format!(r"\\.\pipe\jase-test-{}-{n}", std::process::id()))
}

/// Una conexión cruda, sin el protocolo de `send_to_running`.
#[cfg(not(windows))]
fn raw(ep: &Path) -> std::os::unix::net::UnixStream {
    let s = std::os::unix::net::UnixStream::connect(ep).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    s
}

/// Como `send_to_running`: mientras el servidor crea la instancia siguiente del *pipe*, abrir da
/// `ERROR_PIPE_BUSY` (231) y se reintenta (plazo amplio para el CI).
#[cfg(windows)]
fn raw(ep: &Path) -> std::fs::File {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match std::fs::OpenOptions::new().read(true).write(true).open(ep) {
            Ok(f) => return f,
            Err(e) if e.raw_os_error() == Some(231) && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => panic!("abrir el pipe: {e}"),
        }
    }
}

fn rt() -> Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
}

/// Arranca `serve` en el runtime y espera a que acepte (una línea vacía no se entrega).
fn start(rt: &Runtime, ep: &Path) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel(16);
    let path = ep.to_path_buf();
    rt.spawn(async move { serve(&path, tx).await });
    let deadline = Instant::now() + Duration::from_secs(10);
    while !send_to_running(ep, "") {
        assert!(Instant::now() < deadline, "serve no empezó a escuchar");
        std::thread::sleep(Duration::from_millis(20));
    }
    rx
}

/// La próxima línea recibida (plazo amplio: solo da tiempo a fallar).
fn recv(rt: &Runtime, rx: &mut mpsc::Receiver<String>) -> Option<String> {
    rt.block_on(async { tokio::time::timeout(Duration::from_secs(10), rx.next()).await })
        .ok()
        .flatten()
}

#[test]
fn server_name_has_user_and_safe_chars() {
    assert_eq!(server_name(Some("ana/b c")), "jase-link-ana_b_c");
    assert_eq!(server_name(Some("ñu")), "jase-link-_u");
    let long = "x".repeat(100);
    assert_eq!(
        server_name(Some(&long)),
        format!("jase-link-{}", "x".repeat(64))
    );
    assert_eq!(server_name(None), "jase-link-user");
    assert_eq!(server_name(Some("")), "jase-link-user");
}

#[test]
fn send_without_server_returns_false() {
    let dir = tempfile::tempdir().unwrap();
    let start = Instant::now();
    assert!(!send_to_running(&test_endpoint(dir.path()), "jase://x"));
    assert!(start.elapsed() < SEND_TIMEOUT + Duration::from_millis(100));
}

#[test]
fn send_delivers_lines() {
    let dir = tempfile::tempdir().unwrap();
    let ep = test_endpoint(dir.path());
    let rt = rt();
    let mut rx = start(&rt, &ep);
    let url = "jase://connect/cod2/1.2.3.4:28960";
    assert!(send_to_running(&ep, url));
    assert_eq!(recv(&rt, &mut rx).as_deref(), Some(url));
    assert!(send_to_running(&ep, "show"));
    assert_eq!(recv(&rt, &mut rx).as_deref(), Some("show"));
}

#[test]
fn oversized_input_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let ep = test_endpoint(dir.path());
    let rt = rt();
    let mut rx = start(&rt, &ep);
    {
        let mut c = raw(&ep);
        let _ = c.write_all(&[b'x'; 2000]); // el servidor puede cerrar antes del final
    }
    // Lo que llega después es lo siguiente: la de 2.000 bytes no entregó nada.
    let exact = "a".repeat(MAX_BYTES);
    assert!(send_to_running(&ep, &exact));
    assert_eq!(recv(&rt, &mut rx), Some(exact));
}

#[test]
fn listen_refuses_when_another_instance_is_alive() {
    let dir = tempfile::tempdir().unwrap();
    let ep = test_endpoint(dir.path());
    let rt = rt();
    let mut rx = start(&rt, &ep);
    let (tx, _rx2) = mpsc::channel(1);
    let second =
        rt.block_on(async { tokio::time::timeout(Duration::from_secs(10), serve(&ep, tx)).await });
    assert!(matches!(second, Ok(Err(_))));
    assert!(send_to_running(&ep, "jase://x"));
    assert_eq!(recv(&rt, &mut rx).as_deref(), Some("jase://x"));
}

#[cfg(unix)]
#[test]
fn listen_reclaims_dead_socket() {
    let dir = tempfile::tempdir().unwrap();
    let ep = test_endpoint(dir.path());
    drop(std::os::unix::net::UnixListener::bind(&ep).unwrap()); // deja el archivo
    assert!(ep.exists());
    let rt = rt();
    let mut rx = start(&rt, &ep);
    assert!(send_to_running(&ep, "show"));
    assert_eq!(recv(&rt, &mut rx).as_deref(), Some("show"));
}

#[cfg(unix)]
#[test]
fn listen_keeps_non_socket_file() {
    let dir = tempfile::tempdir().unwrap();
    let ep = test_endpoint(dir.path());
    std::fs::write(&ep, "no soy un socket").unwrap();
    let (tx, _rx) = mpsc::channel(1);
    let result = rt().block_on(serve(&ep, tx));
    assert!(result.is_err());
    assert_eq!(std::fs::read_to_string(&ep).unwrap(), "no soy un socket");
}

#[cfg(unix)]
#[test]
fn socket_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let ep = test_endpoint(dir.path());
    let rt = rt();
    let _rx = start(&rt, &ep);
    let mode = std::fs::metadata(&ep).unwrap().permissions().mode();
    assert_eq!(mode & 0o077, 0, "modo {mode:o}");
}

/// La ruta `/` es de root: ni se conecta ni se reclama (no es lo mismo que «no es un socket»).
#[cfg(unix)]
#[test]
fn foreign_owner_is_refused() {
    use std::os::unix::fs::MetadataExt;
    if std::fs::metadata("/").unwrap().uid() == unsafe { libc::geteuid() } {
        return; // corriendo como root: `/` es propia
    }
    let (tx, _rx) = mpsc::channel(1);
    let err = rt().block_on(serve(Path::new("/"), tx)).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(!send_to_running(Path::new("/"), "x"));
}

#[test]
fn many_lines_on_one_connection_emit_once() {
    let dir = tempfile::tempdir().unwrap();
    let ep = test_endpoint(dir.path());
    let rt = rt();
    let mut rx = start(&rt, &ep);
    raw(&ep).write_all(b"a\nb\nc\n").unwrap();
    assert_eq!(recv(&rt, &mut rx).as_deref(), Some("a"));
    // Una conexión entrega una sola línea: lo siguiente es lo de otra conexión.
    assert!(send_to_running(&ep, "z"));
    assert_eq!(recv(&rt, &mut rx).as_deref(), Some("z"));
}

#[test]
fn idle_connection_is_closed() {
    let dir = tempfile::tempdir().unwrap();
    let ep = test_endpoint(dir.path());
    let rt = rt();
    let mut rx = start(&rt, &ep);
    let mut c = raw(&ep);
    let start = Instant::now();
    // En un hilo: el *pipe* de Windows no tiene plazo de lectura y una regresión colgaría.
    let (done, read) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8];
        let _ = done.send(c.read(&mut buf).map_err(|e| e.kind()));
    });
    let n = read
        .recv_timeout(Duration::from_secs(10))
        .expect("la conexión ociosa no se cerró");
    assert_eq!(n, Ok(0)); // EOF: el servidor la cerró
    assert!(start.elapsed() < Duration::from_secs(5));
    assert!(send_to_running(&ep, "show"));
    assert_eq!(recv(&rt, &mut rx).as_deref(), Some("show"));
}

#[tokio::test(start_paused = true)]
async fn read_line_cases() {
    async fn feed(data: &[u8], close: bool) -> Option<String> {
        let (mut w, r) = tokio::io::duplex(4096);
        w.write_all(data).await.unwrap();
        if close {
            drop(w);
            read_line(r).await
        } else {
            let line = read_line(r).await;
            drop(w);
            line
        }
    }
    assert_eq!(
        feed(b"ab\xffc\n", false).await.as_deref(),
        Some("ab\u{FFFD}c")
    );
    assert_eq!(feed(b"  hola \t\n", false).await.as_deref(), Some("hola"));
    assert_eq!(feed(b"   \n", false).await, None);
    assert_eq!(feed(b"jase://x", true).await.as_deref(), Some("jase://x"));
    // Sin `\n` y sin cerrar: vence el plazo.
    assert_eq!(feed(b"jase://x", false).await, None);
    let over = vec![b'x'; MAX_BYTES + 1];
    assert_eq!(feed(&over, true).await, None);
}
