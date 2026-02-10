#![recursion_limit = "256"]

use std::thread;

use rs_protocol::protocol::Conn;
use rs_utils::{FromNetMessage, InventoryItemStack, ToNetMessage};

mod chunk_decode;
mod handle_packet;

pub fn start_networking(
    from_main: crossbeam::channel::Receiver<ToNetMessage>,
    to_main: crossbeam::channel::Sender<FromNetMessage>,
) {
    //enable_network_debug();

    if let Ok(msg) = from_main.recv() {
        match msg {
            ToNetMessage::Connect { username, address } => {
                println!("Connecting to server at {} as {}", address, username);
                match connect(&address, &username) {
                    Ok(conn) => {
                        println!("Connected to server");
                        let to_main_signal = to_main.clone();

                        message_receiver_thread(conn.clone(), from_main);
                        to_main_signal.send(FromNetMessage::Connected).unwrap();
                        packet_handler_loop(conn, to_main_signal.clone());
                    }
                    Err(e) => {
                        println!("Failed to connect to server: {}", e);
                        to_main.send(FromNetMessage::Disconnected).unwrap();
                    }
                }
            }
            _ => {
                println!("Received unhandled ToNetMessage");
            }
        }
    }
}

fn message_receiver_thread(mut conn: Conn, from_main: crossbeam::channel::Receiver<ToNetMessage>) {
    thread::spawn(move || {
        while let Ok(msg) = from_main.recv() {
            match msg {
                ToNetMessage::Disconnect => {
                    println!("Received disconnect message");
                    break;
                }
                ToNetMessage::ChatMessage(text) => {
                    conn.write_packet(
                        rs_protocol::protocol::packet::play::serverbound::ChatMessage {
                            message: text,
                        },
                    )
                    .unwrap();
                }
                ToNetMessage::PlayerMove {
                    x,
                    y,
                    z,
                    yaw,
                    pitch,
                    on_ground,
                } => {
                    conn.write_packet(
                        rs_protocol::protocol::packet::play::serverbound::PlayerPositionLook {
                            x,
                            y,
                            z,
                            yaw,
                            pitch,
                            on_ground,
                        },
                    )
                    .unwrap();
                }
                ToNetMessage::Respawn => {
                    let _ = rs_protocol::protocol::packet::send_client_status(
                        &mut conn,
                        rs_protocol::protocol::packet::ClientStatus::PerformRespawn,
                    );
                }
                ToNetMessage::PlayerAction { action_id } => {
                    let _ = conn.write_packet(
                        rs_protocol::protocol::packet::play::serverbound::PlayerAction {
                            entity_id: rs_protocol::protocol::VarInt(0),
                            action_id: rs_protocol::protocol::VarInt(action_id as i32),
                            jump_boost: rs_protocol::protocol::VarInt(0),
                        },
                    );
                }
                ToNetMessage::HeldItemChange { slot } => {
                    let _ = conn.write_packet(
                        rs_protocol::protocol::packet::play::serverbound::HeldItemChange { slot },
                    );
                }
                ToNetMessage::ClickWindow {
                    id,
                    slot,
                    button,
                    mode,
                    action_number,
                    clicked_item,
                } => {
                    let _ = conn.write_packet(
                        rs_protocol::protocol::packet::play::serverbound::ClickWindow_u8 {
                            id,
                            slot,
                            button,
                            mode,
                            action_number,
                            clicked_item: clicked_item.map(to_protocol_stack),
                        },
                    );
                }
                ToNetMessage::ConfirmTransaction {
                    id,
                    action_number,
                    accepted,
                } => {
                    let _ = conn.write_packet(
                        rs_protocol::protocol::packet::play::serverbound::ConfirmTransactionServerbound {
                            id,
                            action_number,
                            accepted,
                        },
                    );
                }
                ToNetMessage::CloseWindow { id } => {
                    let _ = conn.write_packet(
                        rs_protocol::protocol::packet::play::serverbound::CloseWindow { id },
                    );
                }
                ToNetMessage::DigStart { x, y, z, face } => {
                    let _ = conn.write_packet(
                        rs_protocol::protocol::packet::play::serverbound::PlayerDigging_u8 {
                            status: 0,
                            location: rs_protocol::shared::Position::new(x, y, z),
                            face,
                        },
                    );
                }
                ToNetMessage::DigFinish { x, y, z, face } => {
                    let _ = conn.write_packet(
                        rs_protocol::protocol::packet::play::serverbound::PlayerDigging_u8 {
                            status: 2,
                            location: rs_protocol::shared::Position::new(x, y, z),
                            face,
                        },
                    );
                }
                ToNetMessage::PlaceBlock {
                    x,
                    y,
                    z,
                    face,
                    cursor_x,
                    cursor_y,
                    cursor_z,
                } => {
                    let _ = conn.write_packet(
                        rs_protocol::protocol::packet::play::serverbound::PlayerBlockPlacement_u8_Item {
                            location: rs_protocol::shared::Position::new(x, y, z),
                            face,
                            hand: None,
                            cursor_x,
                            cursor_y,
                            cursor_z,
                        },
                    );
                }
                _ => {}
            }
        }
    });
}

fn packet_handler_loop(mut conn: Conn, to_main: crossbeam::channel::Sender<FromNetMessage>) {
    loop {
        match conn.read_packet() {
            Ok(pkt) => {
                // Forward packet to main thread
                handle_packet::handle_packet(pkt, &to_main, &mut conn);
            }
            Err(e) => {
                println!("Error reading packet: {}", e);
                let _ = to_main.send(FromNetMessage::Disconnected);
                break;
            }
        }
    }
}

fn to_protocol_stack(item: InventoryItemStack) -> rs_protocol::item::Stack {
    let mut stack = rs_protocol::item::Stack::default();
    stack.id = item.item_id as isize;
    stack.count = item.count as isize;
    stack.damage = Some(item.damage as isize);
    stack
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
