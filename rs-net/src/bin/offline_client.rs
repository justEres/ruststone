use std::env;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Simple CLI: first arg is host:port (default localhost:25565), second arg is username
    let args: Vec<String> = env::args().collect();
    let target = args.get(1).map(|s| s.as_str()).unwrap_or("127.0.0.1:25565");
    let username = args.get(2).map(|s| s.as_str()).unwrap_or("RustPlayer");

    println!("Connecting to {} as {} (protocol 47)", target, username);

    // Use the protocol crate from workspace
    let mut conn = rs_protocol::protocol::Conn::new(target, 47)?;

    // Send handshake (next = 2 for login)
    conn.write_packet(rs_protocol::protocol::packet::handshake::serverbound::Handshake {
        protocol_version: rs_protocol::protocol::VarInt(47),
        host: conn.host.clone(),
        port: conn.port,
        next: rs_protocol::protocol::VarInt(2),
    })?;

    // Switch to login state
    conn.state = rs_protocol::protocol::State::Login;

    // Send LoginStart with username
    conn.write_packet(rs_protocol::protocol::packet::login::serverbound::LoginStart {
        username: username.to_string(),
    })?;

    // Loop reading packets and printing concise summaries (packet kind + small fields)
    loop {
        match conn.read_packet() {
            Ok(pkt) => {
                // Match a few important login-related packets explicitly and summarise others
                use rs_protocol::protocol::packet::Packet;
                match pkt {
                    Packet::SetInitialCompression(s) => {
                        println!("RECV: SetInitialCompression (threshold={})", s.threshold.0);
                        conn.set_compression(s.threshold.0);
                    }
                    Packet::LoginSuccess_String(s) => {
                        println!("RECV: LoginSuccess_String (uuid={}, username={})", s.uuid, s.username);
                        conn.state = rs_protocol::protocol::State::Play;
                    }
                    Packet::LoginSuccess_UUID(s) => {
                        println!("RECV: LoginSuccess_UUID (uuid=..., username={})", s.username);
                        conn.state = rs_protocol::protocol::State::Play;
                    }
                    // Generic fallback: produce a concise summary using the packet's Debug output.
                    // This avoids having to reference every concrete Packet variant type by name.
                    other => {
                        let dbg = format!("{:?}", other);
                        // Extract variant name (text before first '(') if present
                        let variant = if let Some(idx) = dbg.find('(') {
                            dbg[..idx].to_string()
                        } else {
                            dbg.clone()
                        };

                        // Helper to extract a field value from the Debug string: find "key: value" and return value
                        let extract = |key: &str| -> Option<String> {
                            let needle = format!("{}:", key);
                            if let Some(pos) = dbg.find(&needle) {
                                let start = pos + needle.len();
                                let rest = &dbg[start..];
                                let rest = rest.trim_start();
                                let end = rest.find(|c: char| c == ',' || c == ')' || c == '}').unwrap_or(rest.len());
                                return Some(rest[..end].trim().to_string());
                            }
                            None
                        };

                        let mut parts: Vec<String> = Vec::new();
                        for key in &[
                            "id",
                            "entity_id",
                            "chunk_x",
                            "chunk_z",
                            "world_age",
                            "time_of_day",
                            "x",
                            "y",
                            "z",
                            "health",
                            "food",
                            "food_saturation",
                            "dimension",
                            "ping",
                        ] {
                            if let Some(v) = extract(key) {
                                parts.push(format!("{}={}", key, v));
                            }
                        }

                        if parts.is_empty() {
                            println!("RECV: {}", variant);
                        } else {
                            println!("RECV: {} ({})", variant, parts.join(", "));
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("read_packet error: {:?}", e);
                // brief sleep to avoid tight loop on persistent errors
                std::thread::sleep(Duration::from_millis(200));
                break;
            }
        }
    }

    Ok(())
}
