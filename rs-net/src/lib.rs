#![recursion_limit = "256"]

use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use rand::Rng;
use serde_json::Value;
use rs_protocol::protocol::Conn;
use rs_protocol::protocol::login::{Account, AccountType};
use rs_protocol::protocol::packet::Packet;
use rs_utils::{AuthMode, EntityUseAction, FromNetMessage, InventoryItemStack, ToNetMessage};

mod chunk_decode;
mod handle_packet;

pub fn start_networking(
    from_main: crossbeam::channel::Receiver<ToNetMessage>,
    to_main: crossbeam::channel::Sender<FromNetMessage>,
) {
    if dotenvy::dotenv().is_ok() {
        println!("Loaded environment from .env");
    }

    loop {
        let Some(connect_req) = wait_for_connect_request(&from_main) else {
            break;
        };
        println!(
            "Connecting to server at {} as {} ({:?})",
            connect_req.address, connect_req.username, connect_req.auth_mode
        );
        match connect(
            &connect_req.address,
            &connect_req.username,
            connect_req.auth_mode,
        ) {
            Ok(mut conn) => {
                println!("Connected to server");
                let _ = to_main.send(FromNetMessage::Connected);
                let shutdown = run_connected_session(&mut conn, &from_main, &to_main);
                let _ = to_main.send(FromNetMessage::Disconnected);
                if shutdown {
                    break;
                }
            }
            Err(e) => {
                println!("Failed to connect to server: {}", e);
                let _ = to_main.send(FromNetMessage::Disconnected);
            }
        }
    }
}

#[derive(Debug, Clone)]
struct ConnectRequest {
    username: String,
    address: String,
    auth_mode: AuthMode,
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
            } => {
                return Some(ConnectRequest {
                    username,
                    address,
                    auth_mode,
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
) -> bool {
    let (pkt_tx, pkt_rx) = crossbeam::channel::unbounded::<Result<Packet, String>>();
    let mut reader_conn = conn.clone();
    thread::spawn(move || {
        loop {
            match reader_conn.read_packet() {
                Ok(pkt) => {
                    if pkt_tx.send(Ok(pkt)).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    let _ = pkt_tx.send(Err(e.to_string()));
                    break;
                }
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
                        println!("Received disconnect message");
                        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| conn.close()));
                        return false;
                    }
                    ToNetMessage::Shutdown => {
                        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| conn.close()));
                        return true;
                    }
                    ToNetMessage::Connect { .. } => {
                        // Already connected; ignore duplicate connect requests.
                    }
                    other => send_session_message(conn, other),
                }
            }
            recv(pkt_rx) -> incoming => {
                match incoming {
                    Ok(Ok(pkt)) => handle_packet::handle_packet(pkt, to_main, conn),
                    Ok(Err(err)) => {
                        println!("Error reading packet: {}", err);
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

fn send_session_message(conn: &mut Conn, msg: ToNetMessage) {
    match msg {
        ToNetMessage::ChatMessage(text) => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::ChatMessage { message: text },
            );
        }
        ToNetMessage::PlayerMove {
            x,
            y,
            z,
            yaw,
            pitch,
            on_ground,
        } => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::PlayerPositionLook {
                    x,
                    y,
                    z,
                    yaw,
                    pitch,
                    on_ground,
                },
            );
        }
        ToNetMessage::Respawn => {
            let _ = rs_protocol::protocol::packet::send_client_status(
                conn,
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
        ToNetMessage::SwingArm => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::ArmSwing_Handsfree { empty: () },
            );
        }
        ToNetMessage::UseEntity { target_id, action } => {
            let ty = match action {
                EntityUseAction::Interact => 0,
                EntityUseAction::Attack => 1,
            };
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::UseEntity_Handsfree {
                    target_id: rs_protocol::protocol::VarInt(target_id),
                    ty: rs_protocol::protocol::VarInt(ty),
                    target_x: 0.0,
                    target_y: 0.0,
                    target_z: 0.0,
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
            let _ = conn
                .write_packet(rs_protocol::protocol::packet::play::serverbound::CloseWindow { id });
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
        ToNetMessage::DigCancel { x, y, z, face } => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::PlayerDigging_u8 {
                    status: 1,
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
        ToNetMessage::UseItem { held_item } => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::PlayerBlockPlacement_u8_Item {
                    location: rs_protocol::shared::Position::new(-1, -1, -1),
                    face: -1,
                    hand: held_item.map(to_protocol_stack),
                    cursor_x: 0,
                    cursor_y: 0,
                    cursor_z: 0,
                },
            );
        }
        ToNetMessage::DropHeldItem { full_stack } => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::PlayerDigging_u8 {
                    status: if full_stack { 3 } else { 4 },
                    location: rs_protocol::shared::Position::new(-1, -1, -1),
                    face: 255,
                },
            );
        }
        ToNetMessage::Connect { .. } | ToNetMessage::Disconnect | ToNetMessage::Shutdown => {}
    }
}

fn to_protocol_stack(item: InventoryItemStack) -> rs_protocol::item::Stack {
    let mut stack = rs_protocol::item::Stack::default();
    stack.id = item.item_id as isize;
    stack.count = item.count as isize;
    stack.damage = Some(item.damage as isize);
    stack
}

fn connect(
    target: &str,
    username: &str,
    auth_mode: AuthMode,
) -> Result<Conn, Box<dyn std::error::Error>> {
    let mut conn = Conn::new(target, 47)?;
    let online_account = match auth_mode {
        AuthMode::Offline => None,
        AuthMode::Authenticated => Some(
            load_online_account(username).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Authenticated mode selected but no valid account could be loaded (set env vars or use Prism account)",
                )
            })?,
        ),
    };
    let effective_username = online_account
        .as_ref()
        .map(|account| account.name.clone())
        .unwrap_or_else(|| username.to_string());

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
            username: effective_username,
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
                    Packet::EncryptionRequest(req) => {
                        println!("RECV: EncryptionRequest");
                        handle_encryption_request(
                            &mut conn,
                            &req.server_id,
                            &req.public_key.data,
                            &req.verify_token.data,
                            online_account.as_ref(),
                        )?;
                    }
                    Packet::EncryptionRequest_i16(req) => {
                        println!("RECV: EncryptionRequest_i16");
                        handle_encryption_request(
                            &mut conn,
                            &req.server_id,
                            &req.public_key.data,
                            &req.verify_token.data,
                            online_account.as_ref(),
                        )?;
                    }
                    Packet::LoginDisconnect(disconnect) => {
                        return Err(format!("Login disconnect: {}", disconnect.reason).into());
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

fn load_online_account(preferred_username: &str) -> Option<Account> {
    if let Some(account) = load_online_account_from_prism(preferred_username) {
        return Some(account);
    }
    load_online_account_from_env(preferred_username)
}

fn load_online_account_from_env(preferred_username: &str) -> Option<Account> {
    let username = std::env::var("RS_AUTH_USERNAME")
        .ok()
        .map(|v| v.trim().trim_matches('"').to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| preferred_username.to_string());
    let access_token = std::env::var("RS_AUTH_ACCESS_TOKEN")
        .ok()?
        .trim()
        .trim_matches('"')
        .to_string();
    let mut uuid = std::env::var("RS_AUTH_UUID")
        .ok()?
        .trim()
        .trim_matches('"')
        .to_string();
    uuid.retain(|c| c != '-');
    if uuid.len() != 32 {
        println!("RS_AUTH_UUID is set but invalid (expected 32 hex chars, optionally with '-')");
        return None;
    }
    if token_expired_soon(&access_token, 60) {
        println!("RS_AUTH_ACCESS_TOKEN appears expired/near expiry; falling back to Prism auth");
        return None;
    }
    println!(
        "Auth env loaded: username={} uuid={}.. token_len={}",
        username,
        &uuid[..8],
        access_token.len()
    );
    let mut account = Account::new(username.to_string(), Some(uuid), AccountType::Microsoft);
    // Compatibility with rs-protocol microsoft join_server implementation expecting index 2.
    account.verification_tokens = vec![String::new(), String::new(), access_token];
    Some(account)
}

fn load_online_account_from_prism(preferred_username: &str) -> Option<Account> {
    let path = std::env::var("RS_PRISM_ACCOUNTS_PATH")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_default();
            format!("{home}/.local/share/PrismLauncher/accounts.json")
        });

    let raw = std::fs::read_to_string(&path).ok()?;
    let root: Value = serde_json::from_str(&raw).ok()?;
    let accounts = root.get("accounts")?.as_array()?;
    if accounts.is_empty() {
        return None;
    }

    let selected = accounts
        .iter()
        .find(|acc| {
            acc.get("active")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                && acc.get("type").and_then(Value::as_str) == Some("MSA")
        })
        .or_else(|| {
            accounts.iter().find(|acc| {
                acc.get("type").and_then(Value::as_str) == Some("MSA")
                    && acc
                        .pointer("/profile/name")
                        .and_then(Value::as_str)
                        .map(|n| n.eq_ignore_ascii_case(preferred_username))
                        .unwrap_or(false)
            })
        })
        .or_else(|| {
            accounts
                .iter()
                .find(|acc| acc.get("type").and_then(Value::as_str) == Some("MSA"))
        })?;

    let profile_name = selected.pointer("/profile/name")?.as_str()?.to_string();
    let profile_id = selected.pointer("/profile/id")?.as_str()?.to_string();
    if profile_id.len() != 32 {
        println!("Prism profile id is invalid length for account {}", profile_name);
        return None;
    }

    let mut access_token = selected
        .pointer("/ygg/token")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let ygg_exp = selected.pointer("/ygg/exp").and_then(Value::as_i64);

    if access_token.is_empty()
        || ygg_exp
            .map(|exp| exp <= unix_ts_now() as i64 + 60)
            .unwrap_or(true)
        || token_expired_soon(&access_token, 60)
    {
        let refresh_token = selected
            .pointer("/msa/refresh_token")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let client_id = selected
            .get("msa-client-id")
            .and_then(Value::as_str)
            .unwrap_or("00000000402b5328");
        match refresh_minecraft_access_token(refresh_token, client_id) {
            Ok(token) => {
                access_token = token;
                println!("Refreshed Minecraft access token from Prism account {}", profile_name);
            }
            Err(err) => {
                println!("Failed to refresh Prism account token: {}", err);
                return None;
            }
        }
    }

    if access_token.is_empty() {
        return None;
    }

    println!(
        "Auth Prism loaded: username={} uuid={}.. token_len={}",
        profile_name,
        &profile_id[..8],
        access_token.len()
    );
    let mut account = Account::new(profile_name, Some(profile_id), AccountType::Microsoft);
    account.verification_tokens = vec![String::new(), String::new(), access_token];
    Some(account)
}

fn refresh_minecraft_access_token(
    refresh_token: &str,
    client_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    if refresh_token.is_empty() {
        return Err("missing msa refresh token".into());
    }

    let client = reqwest::blocking::Client::new();

    let oauth: Value = client
        .post("https://login.live.com/oauth20_token.srf")
        .form(&[
            ("client_id", client_id),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("redirect_uri", "https://login.live.com/oauth20_desktop.srf"),
            ("scope", "service::user.auth.xboxlive.com::MBI_SSL"),
        ])
        .send()?
        .error_for_status()?
        .json()?;
    let msa_access = oauth
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or("oauth refresh response missing access_token")?;

    let xbl: Value = client
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&serde_json::json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={}", msa_access),
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        }))
        .send()?
        .error_for_status()?
        .json()?;
    let user_token = xbl
        .get("Token")
        .and_then(Value::as_str)
        .ok_or("xbl response missing Token")?;

    let xsts: Value = client
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&serde_json::json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [user_token],
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT",
        }))
        .send()?
        .error_for_status()?
        .json()?;
    let xsts_token = xsts
        .get("Token")
        .and_then(Value::as_str)
        .ok_or("xsts response missing Token")?;
    let uhs = xsts
        .pointer("/DisplayClaims/xui/0/uhs")
        .and_then(Value::as_str)
        .ok_or("xsts response missing uhs")?;

    let mc_auth: Value = client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&serde_json::json!({
            "identityToken": format!("XBL3.0 x={};{}", uhs, xsts_token),
        }))
        .send()?
        .error_for_status()?
        .json()?;
    let minecraft_access = mc_auth
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or("minecraft login response missing access_token")?;

    Ok(minecraft_access.to_string())
}

fn token_expired_soon(token: &str, leeway_secs: u64) -> bool {
    use base64::Engine;

    let mut parts = token.split('.');
    let _header = parts.next();
    let payload = parts.next();
    if payload.is_none() {
        return false;
    }

    let mut payload = payload.unwrap().to_string();
    let rem = payload.len() % 4;
    if rem != 0 {
        payload.extend(std::iter::repeat_n('=', 4 - rem));
    }

    let Ok(bytes) = base64::engine::general_purpose::URL_SAFE.decode(payload) else {
        return false;
    };
    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
        return false;
    };
    let Some(exp) = value.get("exp").and_then(Value::as_u64) else {
        return false;
    };
    exp <= unix_ts_now() + leeway_secs
}

fn unix_ts_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn handle_encryption_request(
    conn: &mut Conn,
    server_id: &str,
    public_key: &[u8],
    verify_token: &[u8],
    online_account: Option<&Account>,
) -> Result<(), Box<dyn std::error::Error>> {
    let account = online_account.ok_or_else(|| {
        "Server requires online-mode authentication; set RS_AUTH_UUID and RS_AUTH_ACCESS_TOKEN"
            .to_string()
    })?;
    println!(
        "Starting encrypted login: server_id='{}' pubkey_len={} verify_len={} uuid={}..",
        server_id,
        public_key.len(),
        verify_token.len(),
        account
            .uuid
            .as_deref()
            .unwrap_or("<none>")
            .chars()
            .take(8)
            .collect::<String>()
    );

    let mut shared_secret = [0u8; 16];
    rand::thread_rng().fill(&mut shared_secret);

    account.join_server(server_id, &shared_secret, public_key)?;

    let shared_encrypted = rsa_public_encrypt_pkcs1::encrypt(public_key, &shared_secret)?;
    let token_encrypted = rsa_public_encrypt_pkcs1::encrypt(public_key, verify_token)?;

    conn.write_packet(
        rs_protocol::protocol::packet::login::serverbound::EncryptionResponse {
            shared_secret: rs_protocol::protocol::LenPrefixedBytes::new(shared_encrypted),
            verify_token: rs_protocol::protocol::LenPrefixedBytes::new(token_encrypted),
        },
    )?;

    conn.enable_encyption(&shared_secret);
    println!("Encryption enabled");
    Ok(())
}
