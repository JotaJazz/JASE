//! Una sola instancia (`ui/single_instance.py`): la segunda JASE entrega `show` o el enlace a
//! la primera y sale. Mismo punto de encuentro que `QLocalServer` de 1.3.0: un socket Unix en
//! la carpeta temporal o el *named pipe* `\\.\pipe\jase-link-<usuario>` en Windows.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures::channel::mpsc;
use futures::{SinkExt, Stream};
use jase_core::pystr::py_strip;
use tokio::io::{AsyncRead, AsyncReadExt};

/// Lo manda una segunda JASE abierta sin enlace: la que ya corre se trae al frente.
pub const SHOW_COMMAND: &str = "show";
/// Por conexión; lo que sobre se descarta.
pub const MAX_BYTES: usize = 1024;
/// Una conexión que no entrega su línea en este plazo se cierra.
pub const READ_TIMEOUT: Duration = Duration::from_millis(500);
pub const SEND_TIMEOUT: Duration = Duration::from_millis(500);

/// `"jase-link-"` + el usuario con lo que no sea `[A-Za-z0-9_-]` cambiado por `_` (64 como
/// mucho); `"user"` si no hay.
pub fn server_name(user: Option<&str>) -> String {
    let safe: String = user
        .unwrap_or("")
        .chars()
        .take(64)
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("jase-link-{}", if safe.is_empty() { "user" } else { &safe })
}

/// El usuario como `getpass.getuser()`: la primera variable con valor.
pub fn current_user() -> Option<String> {
    ["LOGNAME", "USER", "LNAME", "USERNAME"]
        .into_iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
}

/// Lee la primera línea (como mucho `MAX_BYTES`) en `READ_TIMEOUT`; si cierra antes, todo lo
/// leído. `None` si vence, si pasa del tope sin `\n` o si queda vacía.
pub async fn read_line<R: AsyncRead + Unpin>(r: R) -> Option<String> {
    let mut buf = Vec::with_capacity(MAX_BYTES + 1);
    let mut r = r.take(MAX_BYTES as u64 + 1);
    let read = async {
        loop {
            let n = r.read_buf(&mut buf).await.ok()?;
            if let Some(nl) = buf.iter().position(|&b| b == b'\n') {
                return Some(nl);
            }
            if n == 0 {
                return (buf.len() <= MAX_BYTES).then_some(buf.len());
            }
        }
    };
    let end = tokio::time::timeout(READ_TIMEOUT, read).await.ok()??;
    let text = String::from_utf8_lossy(&buf[..end]);
    let line = py_strip(&text);
    (!line.is_empty()).then(|| line.to_owned())
}

/// Lee una línea de la conexión y la entrega (una por conexión; el resto se descarta).
async fn deliver<R: AsyncRead + Unpin>(conn: R, mut tx: mpsc::Sender<String>) {
    if let Some(line) = read_line(conn).await {
        let _ = tx.send(line).await;
    }
}

/// El socket de la primera JASE como `Stream` de líneas (para `Subscription::run_with`); si
/// no se pudo escuchar, queda pendiente para siempre: sin instancia única, como en Python.
#[allow(clippy::ptr_arg)] // `run_with` pide `fn(&D)` con el `PathBuf` que guarda
pub fn listen(endpoint: &PathBuf) -> impl Stream<Item = String> + use<> {
    let endpoint = endpoint.clone();
    iced::stream::channel(16, async move |tx| {
        let _ = serve(&endpoint, tx).await;
        std::future::pending::<()>().await;
    })
}

#[cfg(not(windows))]
fn owned_by_me(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    // SAFETY: `geteuid` no falla ni toca memoria.
    meta.uid() == unsafe { libc::geteuid() }
}

/// En la carpeta temporal (`QDir::tempPath()`), la misma ruta que 1.3.0.
#[cfg(not(windows))]
pub fn endpoint() -> PathBuf {
    std::env::temp_dir().join(server_name(current_user().as_deref()))
}

#[cfg(windows)]
pub fn endpoint() -> PathBuf {
    PathBuf::from(format!(
        r"\\.\pipe\{}",
        server_name(current_user().as_deref())
    ))
}

/// Entrega `line` a la instancia en marcha; `false` si no hay ninguna (nunca se cuelga). La
/// carpeta temporal es compartida: solo se conecta a un socket del mismo usuario.
#[cfg(not(windows))]
pub fn send_to_running(endpoint: &Path, line: &str) -> bool {
    use std::io::Write;
    use std::net::Shutdown;
    use std::os::unix::net::UnixStream;
    if !std::fs::symlink_metadata(endpoint).is_ok_and(|m| owned_by_me(&m)) {
        return false;
    }
    let Ok(mut s) = UnixStream::connect(endpoint) else {
        return false;
    };
    s.set_write_timeout(Some(SEND_TIMEOUT)).is_ok()
        && s.write_all(format!("{line}\n").as_bytes()).is_ok()
        && s.shutdown(Shutdown::Write).is_ok()
}

#[cfg(windows)]
pub fn send_to_running(endpoint: &Path, line: &str) -> bool {
    use std::io::Write;
    use std::time::Instant;
    use windows_sys::Win32::Foundation::ERROR_PIPE_BUSY;
    let deadline = Instant::now() + SEND_TIMEOUT;
    loop {
        match std::fs::OpenOptions::new().write(true).open(endpoint) {
            Ok(mut pipe) => return pipe.write_all(format!("{line}\n").as_bytes()).is_ok(),
            // Todas las instancias del *pipe* ocupadas: se reintenta hasta el plazo.
            Err(e)
                if e.raw_os_error() == Some(ERROR_PIPE_BUSY as i32)
                    && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return false,
        }
    }
}

/// Escucha en `endpoint` y entrega cada línea por `tx`. `Err` si otra JASE viva ya escucha,
/// si la ruta es de otro usuario o no es un socket (nunca se le quita ni se borra).
#[cfg(not(windows))]
pub async fn serve(endpoint: &Path, tx: mpsc::Sender<String>) -> io::Result<()> {
    use std::os::unix::fs::{FileTypeExt, PermissionsExt};
    use tokio::net::UnixListener;
    match std::fs::symlink_metadata(endpoint) {
        Ok(meta) if !owned_by_me(&meta) => {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "ajeno"));
        }
        Ok(meta) if !meta.file_type().is_socket() => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "no es un socket",
            ));
        }
        Ok(_) => {
            if std::os::unix::net::UnixStream::connect(endpoint).is_ok() {
                return Err(io::Error::new(io::ErrorKind::AddrInUse, "otra JASE"));
            }
            std::fs::remove_file(endpoint)?; // socket viejo de una sesión caída
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let listener = UnixListener::bind(endpoint)?;
    // shortcut: unos instantes entre `bind` y `chmod` con los permisos del umask (cambiarlo
    // es global al proceso); crear en una carpeta privada y renombrar si hiciera falta.
    std::fs::set_permissions(endpoint, std::fs::Permissions::from_mode(0o600))?;
    loop {
        match listener.accept().await {
            Ok((conn, _)) => drop(tokio::spawn(deliver(conn, tx.clone()))),
            // Sin descriptores libres, p. ej.: se reintenta sin girar en vacío.
            Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }
}

/// Como el ejemplo de `ServerOptions` de tokio: siempre hay una instancia del *pipe* libre
/// antes de pasar la conectada a su tarea; `first_pipe_instance` falla si otra JASE (también
/// 1.3.0) ya lo creó.
#[cfg(windows)]
pub async fn serve(endpoint: &Path, tx: mpsc::Sender<String>) -> io::Result<()> {
    use tokio::net::windows::named_pipe::ServerOptions;
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(endpoint)?;
    loop {
        let connected = server.connect().await;
        let client = server;
        server = ServerOptions::new()
            .reject_remote_clients(true)
            .create(endpoint)?;
        // Un cliente que ya se fue (`ERROR_NO_DATA`) solo pierde su instancia; sin girar en
        // vacío si el fallo se repite.
        match connected {
            Ok(()) => drop(tokio::spawn(deliver(client, tx.clone()))),
            Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }
}
