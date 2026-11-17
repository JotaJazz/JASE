//! Consulta A2S de Valve: challenge, multipaquete y la 0x49 que completa a una 0x6D.
use super::query::{Datagram, Router, ms};
use crate::models::{ServerAddress, ServerInfo};
use crate::protocols::ProtocolError;
use crate::protocols::valve::{self, PacketAssembler};
use std::net::SocketAddrV4;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::timeout;

/// Límite de S2C_CHALLENGE seguidos por intento (un servidor que siempre responde con
/// challenge no debe provocar un bucle de envíos).
const VALVE_MAX_CHALLENGES: u32 = 2;

/// Un intento A2S: envía `build(None)`, reensambla, responde cada challenge (hasta el
/// límite) reenviando con la hora en `last_sent` y devuelve el primer mensaje de un tipo
/// de `wanted` con su hora. Los demás mensajes completos van a `others`. None si vence.
#[allow(clippy::too_many_arguments)]
async fn a2s(
    router: &Router,
    key: SocketAddrV4,
    rx: &mut UnboundedReceiver<Datagram>,
    timeout_each: Duration,
    build: fn(Option<&[u8]>) -> Vec<u8>,
    wanted: &[u8],
    last_sent: &mut Instant,
    others: &mut Vec<Vec<u8>>,
) -> Option<Result<Datagram, ProtocolError>> {
    let mut assembler = PacketAssembler::new();
    let mut challenges = 0;
    *last_sent = Instant::now();
    router.send(&build(None), key).await;
    let reply = timeout(timeout_each, async {
        while let Some((data, at)) = rx.recv().await {
            let message = match assembler.feed(&data) {
                Ok(Some(m)) if m.len() >= 5 => m,
                Ok(_) => continue,
                Err(e) => return Some(Err(e)),
            };
            if let Some(c) = valve::challenge_of(&message) {
                if challenges < VALVE_MAX_CHALLENGES {
                    challenges += 1;
                    *last_sent = Instant::now();
                    router.send(&build(Some(&c)), key).await;
                }
            } else if wanted.contains(&message[4]) {
                return Some(Ok((message, at)));
            } else {
                others.push(message);
            }
        }
        None
    });
    reply.await.ok().flatten()
}

/// A2S_INFO (con reintentos y challenge) y luego A2S_PLAYER (un intento, opcional).
///
/// El ping va del último envío (el reenvío con challenge, si lo hubo) a la respuesta
/// INFO, para no sumar la ida y vuelta del challenge; en un reintento, desde el primer
/// envío, como en las otras familias.
pub(super) async fn query_valve(
    router: &Router,
    address: &ServerAddress,
    key: SocketAddrV4,
    timeout_each: Duration,
    retries: u32,
    expected_gamedir: &str,
) -> Option<ServerInfo> {
    // Una sola ruta para toda la consulta: lo que llegue tarde a un intento lo ve el
    // siguiente (o la fase de jugadores). Sin return entre register y unregister.
    let mut rx = router.register(key);
    let info = query(
        router,
        address,
        key,
        &mut rx,
        timeout_each,
        retries,
        expected_gamedir,
    )
    .await;
    router.unregister(key);
    info
}

async fn query(
    router: &Router,
    address: &ServerAddress,
    key: SocketAddrV4,
    rx: &mut UnboundedReceiver<Datagram>,
    timeout_each: Duration,
    retries: u32,
    expected_gamedir: &str,
) -> Option<ServerInfo> {
    // Algunos HLDS responden 0x6D y 0x49 a la misma consulta, en cualquier orden y casi
    // a la vez. Si gana la 0x6D (sin versión), la 0x49 que llegue después la completa.
    let mut others = Vec::new();
    let wanted = [valve::INFO_SOURCE, valve::INFO_GOLDSRC];
    let first = Instant::now();
    let mut last_sent = first;
    let mut reply = None;
    for attempt in 0..=retries {
        let build = valve::build_info_query;
        match a2s(
            router,
            key,
            rx,
            timeout_each,
            build,
            &wanted,
            &mut last_sent,
            &mut others,
        )
        .await
        {
            Some(Ok(r)) => {
                reply = Some((r, attempt));
                break;
            }
            Some(Err(_)) => return None,
            None => continue,
        }
    }
    let ((payload, received), attempt) = reply?;
    let start = if attempt == 0 { last_sent } else { first };
    let ping = ms(received.saturating_duration_since(start));
    let mut info = valve::parse_info(&payload, address, ping, expected_gamedir).ok()?;

    let build = valve::build_player_query;
    let wanted = [valve::PLAYER_RESPONSE];
    let mut unused = Instant::now();
    let players = a2s(
        router,
        key,
        rx,
        timeout_each,
        build,
        &wanted,
        &mut unused,
        &mut others,
    );
    // Sin lista de jugadores el servidor sigue siendo válido.
    if let Some(Ok((data, _))) = players.await {
        info.players = valve::parse_players(&data).unwrap_or_default();
    }
    if payload[4] == valve::INFO_GOLDSRC
        && let Some(source) = others.iter().find(|m| m[4] == valve::INFO_SOURCE)
        && let Ok(mut source) = valve::parse_info(source, address, info.ping_ms, expected_gamedir)
    {
        source.players = std::mem::take(&mut info.players);
        info = source;
    }
    Some(info)
}
