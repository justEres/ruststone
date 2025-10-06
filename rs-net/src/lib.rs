#![recursion_limit = "256"]

use std::thread;

use rs_protocol::protocol::{enable_network_debug, packet::Packet, Conn};
use rs_utils::{FromNetMessage, ToNetMessage};

pub fn start_networking(
    from_main: crossbeam::channel::Receiver<ToNetMessage>,
    to_main: crossbeam::channel::Sender<FromNetMessage>,
) {
    enable_network_debug();
    while let Ok(msg) = from_main.recv() {
        match msg {
            ToNetMessage::Connect { username, address } => {
                println!("Connecting to server at {} as {}", address, username);
                match connect(&address, &username) {
                    Ok(conn) => {
                        println!("Connected to server");
                        let to_main_thread = to_main.clone();
                        let to_main_signal = to_main.clone();
                        thread::spawn(move || packet_handler_loop(conn, to_main_thread));
                        to_main_signal.send(FromNetMessage::Connected).unwrap();
                    }
                    Err(e) => {
                        println!("Failed to connect to server: {}", e);
                        to_main.send(FromNetMessage::Disconnected).unwrap();
                    }
                }
            }
            ToNetMessage::Shutdown => {
                // Optionally handle shutdown logic here
                break;
            }
            _ => {
                println!("Received unhandled ToNetMessage");
            }
        }
    }
}


fn packet_handler_loop(mut conn: Conn, to_main: crossbeam::channel::Sender<FromNetMessage>) {
    loop {
        match conn.read_packet() {
            Ok(pkt) => {
                // Forward packet to main thread
                if to_main.send(FromNetMessage::Packet(pkt)).is_err() {
                    // Main thread hung up
                    break;
                }
            }
            Err(e) => {
                println!("Error reading packet: {}", e);
                let _ = to_main.send(FromNetMessage::Disconnected);
                break;
            }
        }
    }
}

fn connect(target: &str, username: &str) -> Result<Conn, Box<dyn std::error::Error>> {
    let mut conn = Conn::new(target, 47)?;

    conn.write_packet(
        rs_protocol::protocol::packet::handshake::serverbound::Handshake {
            protocol_version: rs_protocol::protocol::VarInt(47),
            host: conn.host.clone(),
            port: conn.port,
            next: rs_protocol::protocol::VarInt(2),
        },
    )?;

    conn.state = rs_protocol::protocol::State::Login;

    conn.write_packet(
        rs_protocol::protocol::packet::login::serverbound::LoginStart {
            username: username.to_string(),
        },
    )?;

    loop {
        match conn.read_packet() {
            Ok(pkt) => {
                use rs_protocol::protocol::packet::Packet;
                match pkt {
                    Packet::SetInitialCompression(s) => {
                        println!("RECV: SetInitialCompression (threshold={})", s.threshold.0);
                        conn.set_compression(s.threshold.0);
                    }
                    Packet::LoginSuccess_String(s) => {
                        println!(
                            "RECV: LoginSuccess_String (uuid={}, username={})",
                            s.uuid, s.username
                        );
                        conn.state = rs_protocol::protocol::State::Play;
                        return Ok(conn);
                    }
                    Packet::LoginSuccess_UUID(s) => {
                        println!(
                            "RECV: LoginSuccess_UUID (uuid=..., username={})",
                            s.username
                        );
                        conn.state = rs_protocol::protocol::State::Play;
                        return Ok(conn);
                    }
                    other => {
                        let dbg = format!("{:?}", other);
                        let variant = if let Some(idx) = dbg.find('(') {
                            dbg[..idx].to_string()
                        } else {
                            dbg.clone()
                        };
                        println!("RECV: {} (full={})", variant, dbg);
                    }
                }
            }
            Err(e) => {
                println!("Error reading packet: {}", e);
                return Err(Box::new(e));
            }
        }
    }
}
