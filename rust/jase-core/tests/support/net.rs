#![allow(dead_code)]
//! Servidor UDP falso en 127.0.0.1: responde a cada datagrama con lo que devuelva `handler`.
use std::sync::{Arc, Mutex};
use tokio::net::UdpSocket;
use tokio::task::JoinHandle;

pub struct FakeUdp {
    pub port: u16,
    pub received: Arc<Mutex<Vec<Vec<u8>>>>,
    task: JoinHandle<()>,
}

impl FakeUdp {
    pub fn received(&self) -> Vec<Vec<u8>> {
        self.received.lock().unwrap().clone()
    }
}

impl Drop for FakeUdp {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub async fn fake_udp(handler: impl Fn(&[u8]) -> Vec<Vec<u8>> + Send + 'static) -> FakeUdp {
    let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = sock.local_addr().unwrap().port();
    let received = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&received);
    let task = tokio::spawn(async move {
        let mut buf = vec![0u8; 65535];
        while let Ok((n, from)) = sock.recv_from(&mut buf).await {
            log.lock().unwrap().push(buf[..n].to_vec());
            for reply in handler(&buf[..n]) {
                let _ = sock.send_to(&reply, from).await;
            }
        }
    });
    FakeUdp {
        port,
        received,
        task,
    }
}
