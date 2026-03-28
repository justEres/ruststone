use std::thread;

use rs_protocol::protocol::packet::Packet;
use rs_protocol::protocol::{packet, Conn, Direction, State};
use rs_utils::{AuthMode, FromNetMessage, ToNetMessage};
use tracing::{error, info, warn};

use crate::{auth, handle_packet, outbound};

#[derive(Debug, Clone)]
struct ConnectRequest {
    username: String,
    address: String,
    auth_mode: AuthMode,
    auth_account_uuid: Option<String>,
    prism_accounts_path: Option<String>,
    requested_view_distance: u8,
}

pub fn start_networking(
    from_main: crossbeam::channel::Receiver<ToNetMessage>,
    to_main: crossbeam::channel::Sender<FromNetMessage>,
) {
    if dotenvy::dotenv().is_ok() {
        info!("Loaded environment from .env");
    }

    loop {
        let Some(connect_req) = wait_for_connect_request(&from_main) else {
            break;
        };
        info!(
            "Connecting to server at {} as {} ({:?})",
            connect_req.address, connect_req.username, connect_req.auth_mode
        );
        match auth::connect(
            &connect_req.address,
            &connect_req.username,
            connect_req.auth_mode,
            connect_req.auth_account_uuid.as_deref(),
            connect_req.prism_accounts_path.as_deref(),
        ) {
            Ok(mut conn) => {
                info!("Connected to server");
                let _ = to_main.send(FromNetMessage::Connected);
                let shutdown = run_connected_session(
                    &mut conn,
                    &from_main,
                    &to_main,
                    connect_req.requested_view_distance,
                );
                let _ = to_main.send(FromNetMessage::Disconnected);
                if shutdown {
                    break;
                }
            }
            Err(e) => {
                error!("Failed to connect to server: {}", e);
                let _ = to_main.send(FromNetMessage::Disconnected);
            }
        }
    }
}

fn wait_for_connect_request(
    from_main: &crossbeam::channel::Receiver<ToNetMessage>,
) -> Option<ConnectRequest> {
    loop {
        let Ok(msg) = from_main.recv() else {
            return None;
        };
        match msg {
            ToNetMessage::Connect {
                username,
                address,
                auth_mode,
                auth_account_uuid,
                prism_accounts_path,
                requested_view_distance,
            } => {
                return Some(ConnectRequest {
                    username,
                    address,
                    auth_mode,
                    auth_account_uuid,
                    prism_accounts_path,
                    requested_view_distance,
                });
            }
            ToNetMessage::Shutdown => return None,
            _ => {}
        }
    }
}

fn run_connected_session(
    conn: &mut Conn,
    from_main: &crossbeam::channel::Receiver<ToNetMessage>,
    to_main: &crossbeam::channel::Sender<FromNetMessage>,
    requested_view_distance: u8,
) -> bool {
    let (pkt_tx, pkt_rx) = crossbeam::channel::unbounded::<Result<Packet, String>>();
    let mut reader_conn = conn.clone();
    thread::spawn(move || loop {
        match read_packet_allow_visual_tolerance(&mut reader_conn) {
            Ok(Some(pkt)) => {
                if pkt_tx.send(Ok(pkt)).is_err() {
                    break;
                }
            }
            Ok(None) => continue,
            Err(e) => {
                let _ = pkt_tx.send(Err(e));
                break;
            }
        }
    });

    loop {
        crossbeam::select! {
            recv(from_main) -> msg => {
                let Ok(msg) = msg else {
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| conn.close()));
                    return true;
                };
                match msg {
                    ToNetMessage::Disconnect => {
                        info!("Received disconnect message");
                        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| conn.close()));
                        return false;
                    }
                    ToNetMessage::Shutdown => {
                        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| conn.close()));
                        return true;
                    }
                    ToNetMessage::Connect { .. } => {}
                    other => outbound::send_session_message(conn, other),
                }
            }
            recv(pkt_rx) -> incoming => {
                match incoming {
                    Ok(Ok(pkt)) => {
                        handle_packet::handle_packet(pkt, to_main, conn, requested_view_distance)
                    }
                    Ok(Err(err)) => {
                        warn!("Error reading packet: {}", err);
                        return false;
                    }
                    Err(_) => {
                        return false;
                    }
                }
            }
        }
    }
}

fn read_packet_allow_visual_tolerance(conn: &mut Conn) -> Result<Option<Packet>, String> {
    let compression_threshold = conn.compression_threshold;
    let (id, mut buf) =
        Conn::read_raw_packet_from(conn, compression_threshold).map_err(|err| err.to_string())?;

    let dir = Direction::Clientbound;
    let tolerate_visual = is_tolerable_visual_packet(conn.state, dir, id);

    let packet = match packet::packet_by_id(conn.protocol_version, conn.state, dir, id, &mut buf) {
        Ok(packet) => packet,
        Err(err) if tolerate_visual => {
            warn!(
                "Skipping visual packet 0x{:X} after parse failure: {}",
                id, err
            );
            return Ok(None);
        }
        Err(err) => return Err(err.to_string()),
    };

    let Some(packet) = packet else {
        if tolerate_visual {
            warn!("Skipping unknown visual packet 0x{:X}", id);
            return Ok(None);
        }
        return Err("protocol error: missing packet".to_string());
    };

    let pos = buf.position() as usize;
    let ibuf = buf.into_inner();
    if ibuf.len() != pos {
        let bytes_left = ibuf.len() - pos;
        if tolerate_visual {
            warn!(
                "Accepted visual packet 0x{:X} with {} unread bytes left",
                id, bytes_left
            );
            return Ok(Some(packet));
        }
        return Err(format!(
            "protocol error: Failed to read all of packet 0x{:X}, had {} bytes left",
            id, bytes_left
        ));
    }

    Ok(Some(packet))
}

fn is_tolerable_visual_packet(state: State, dir: Direction, id: i32) -> bool {
    state == State::Play
        && matches!(dir, Direction::Clientbound)
        && matches!(id, 0x3B | 0x3C | 0x3D | 0x3E | 0x45 | 0x47)
}
