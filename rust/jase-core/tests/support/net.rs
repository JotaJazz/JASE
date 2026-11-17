#![allow(dead_code)]
//! Servidor UDP falso en 127.0.0.1: responde a cada datagrama con lo que devuelva `handler`.
use futures::FutureExt;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::task::JoinHandle;

type Log = Arc<Mutex<Vec<(SocketAddr, Vec<u8>)>>>;

pub struct FakeUdp {
    pub port: u16,
    log: Log,
    task: JoinHandle<()>,
}

impl FakeUdp {
    /// Datagramas recibidos, en orden.
    pub fn received(&self) -> Vec<Vec<u8>> {
        self.log
            .lock()
            .unwrap()
            .iter()
            .map(|(_, d)| d.clone())
            .collect()
    }

    /// Origen de cada datagrama recibido.
    pub fn senders(&self) -> Vec<SocketAddr> {
        self.log.lock().unwrap().iter().map(|(a, _)| *a).collect()
    }
}

impl Drop for FakeUdp {
    fn drop(&mut self) {
        // Un pánico del handler (p. ej. un assert) hace fallar el test, no parecer un timeout.
        if let Some(Err(e)) = (&mut self.task).now_or_never()
            && e.is_panic()
            && !std::thread::panicking()
        {
            std::panic::resume_unwind(e.into_panic());
        }
        self.task.abort();
    }
}

pub async fn fake_udp(handler: impl Fn(&[u8]) -> Vec<Vec<u8>> + Send + 'static) -> FakeUdp {
    fake_udp_delayed(move |d| {
        handler(d)
            .into_iter()
            .map(|r| (Duration::ZERO, r))
            .collect()
    })
    .await
}

/// Como `fake_udp`, pero cada respuesta sale tras su demora.
pub async fn fake_udp_delayed(
    handler: impl Fn(&[u8]) -> Vec<(Duration, Vec<u8>)> + Send + 'static,
) -> FakeUdp {
    let sock = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
    let port = sock.local_addr().unwrap().port();
    let log: Log = Arc::default();
    let task_log = Arc::clone(&log);
    let task = tokio::spawn(async move {
        let mut buf = vec![0u8; 65535];
        while let Ok((n, from)) = sock.recv_from(&mut buf).await {
            task_log.lock().unwrap().push((from, buf[..n].to_vec()));
            for (delay, reply) in handler(&buf[..n]) {
                if delay.is_zero() {
                    let _ = sock.send_to(&reply, from).await;
                } else {
                    let sock = Arc::clone(&sock);
                    tokio::spawn(async move {
                        tokio::time::sleep(delay).await;
                        let _ = sock.send_to(&reply, from).await;
                    });
                }
            }
        }
    });
    FakeUdp { port, log, task }
}
