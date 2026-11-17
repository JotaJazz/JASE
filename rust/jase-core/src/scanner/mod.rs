//! Escáner de servidores. Traducción de `src/jase/scanner.py`.
pub mod masters_tcp;
pub mod masters_udp;
pub mod query;

use crate::protocols::ProtocolError;
use std::io;
use std::net::SocketAddrV4;

/// Por qué falló pedir la lista a un master.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MasterFetchError {
    #[error("el master no respondió a tiempo")]
    Timeout,
    #[error("{0}")]
    Network(String),
    #[error("{0}")]
    Protocol(ProtocolError),
    /// El master de Zandronum ignoró la consulta por llegar demasiado pronto.
    #[error("el master está ocupado")]
    Busy,
    #[error("{message}")]
    SteamWeb { kind: &'static str, message: String },
}

impl MasterFetchError {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::Network(_) => "network",
            Self::Protocol(_) => "protocol",
            Self::Busy => "busy",
            Self::SteamWeb { kind, .. } => kind,
        }
    }
}

impl From<ProtocolError> for MasterFetchError {
    fn from(e: ProtocolError) -> Self {
        match e {
            ProtocolError::MasterBusy(_) => Self::Busy,
            e => Self::Protocol(e),
        }
    }
}

impl From<io::Error> for MasterFetchError {
    fn from(e: io::Error) -> Self {
        Self::Network(e.to_string())
    }
}

/// Primer IPv4 que resuelve `host` (el escáner solo habla IPv4).
pub async fn resolve_v4(host: &str, port: u16) -> io::Result<SocketAddrV4> {
    tokio::net::lookup_host((host, port))
        .await?
        .find_map(|a| match a {
            std::net::SocketAddr::V4(v4) => Some(v4),
            std::net::SocketAddr::V6(_) => None,
        })
        .ok_or_else(|| io::Error::other(format!("No se pudo resolver {host}")))
}
