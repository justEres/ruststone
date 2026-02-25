#![recursion_limit = "256"]

use std::path::PathBuf;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use minecraft_msa_auth::MinecraftAuthorizationFlow;
use rand::Rng;
use rs_protocol::protocol::Conn;
use rs_protocol::protocol::login::{Account, AccountType};
use rs_protocol::protocol::packet::Packet;
use rs_utils::{AuthMode, EntityUseAction, FromNetMessage, InventoryItemStack, ToNetMessage};
use serde::Deserialize;
use serde_json::Value;

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
            connect_req.auth_account_uuid.as_deref(),
            connect_req.prism_accounts_path.as_deref(),
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
    auth_account_uuid: Option<String>,
    prism_accounts_path: Option<String>,
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
            } => {
                return Some(ConnectRequest {
                    username,
                    address,
                    auth_mode,
                    auth_account_uuid,
                    prism_accounts_path,
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
        ToNetMessage::PlayerMovePosLook {
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
        ToNetMessage::PlayerMovePos { x, y, z, on_ground } => {
            let _ = conn.write_packet(rs_protocol::protocol::packet::play::serverbound::PlayerPosition {
                x,
                y,
                z,
                on_ground,
            });
        }
        ToNetMessage::PlayerMoveLook {
            yaw,
            pitch,
            on_ground,
        } => {
            let _ = conn.write_packet(rs_protocol::protocol::packet::play::serverbound::PlayerLook {
                yaw,
                pitch,
                on_ground,
            });
        }
        ToNetMessage::PlayerMoveGround { on_ground } => {
            let _ = conn.write_packet(rs_protocol::protocol::packet::play::serverbound::Player {
                on_ground,
            });
        }
        ToNetMessage::Respawn => {
            let _ = rs_protocol::protocol::packet::send_client_status(
                conn,
                rs_protocol::protocol::packet::ClientStatus::PerformRespawn,
            );
        }
        ToNetMessage::PlayerAction {
            entity_id,
            action_id,
        } => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::PlayerAction {
                    entity_id: rs_protocol::protocol::VarInt(entity_id),
                    action_id: rs_protocol::protocol::VarInt(action_id as i32),
                    jump_boost: rs_protocol::protocol::VarInt(0),
                },
            );
        }
        ToNetMessage::ClientAbilities {
            flags,
            flying_speed,
            walking_speed,
        } => {
            let _ = conn.write_packet(
                rs_protocol::protocol::packet::play::serverbound::ClientAbilities_f32 {
                    flags,
                    flying_speed,
                    walking_speed,
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
    auth_account_uuid: Option<&str>,
    prism_accounts_path: Option<&str>,
) -> Result<Conn, Box<dyn std::error::Error>> {
    let mut conn = Conn::new(target, 47)?;
    let online_account = match auth_mode {
        AuthMode::Offline => None,
        AuthMode::Authenticated => {
            let prism_path = prism_accounts_path
                .map(PathBuf::from)
                .unwrap_or_else(default_prism_accounts_pathbuf);
            let prism_path = prism_path.to_string_lossy();
            Some(
                load_online_account_from_prism(&prism_path, auth_account_uuid).map_err(|err| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("Prism authentication failed: {err}"),
                    )
                })?,
            )
        }
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

const MSA_TOKEN_URL_V2: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const MSA_SCOPE: &str = "XboxLive.signin offline_access";
const MINECRAFT_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

fn default_prism_accounts_pathbuf() -> PathBuf {
    if let Ok(home) = std::env::var("HOME")
        && !home.trim().is_empty()
    {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("PrismLauncher")
            .join("accounts.json");
    }
    PathBuf::from("accounts.json")
}

#[derive(Debug)]
struct PrismAuthSelection {
    username: String,
    uuid: String,
    msa_client_id: String,
    refresh_token: String,
    ygg_token: Option<String>,
    ygg_exp: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct MicrosoftTokenResponseV2 {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct MinecraftProfileResponse {
    id: String,
    name: String,
}

fn load_online_account_from_prism(
    prism_path: &str,
    selected_uuid: Option<&str>,
) -> Result<Account, Box<dyn std::error::Error>> {
    let selection = select_prism_account(prism_path, selected_uuid)?;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let minecraft_access_token = runtime.block_on(get_minecraft_access_token(&selection))?;

    println!(
        "Auth Prism: username={} uuid={}.. token_len={}",
        selection.username,
        &selection.uuid[..8],
        minecraft_access_token.len()
    );

    let mut account = Account::new(
        selection.username,
        Some(selection.uuid),
        AccountType::Microsoft,
    );
    // Compatibility with rs-protocol microsoft join_server implementation expecting index 2.
    account.verification_tokens = vec![String::new(), String::new(), minecraft_access_token];
    Ok(account)
}

async fn get_minecraft_access_token(
    selection: &PrismAuthSelection,
) -> Result<String, Box<dyn std::error::Error>> {
    if let (Some(token), Some(exp)) = (&selection.ygg_token, selection.ygg_exp) {
        if exp > unix_now() + 60 && !token.trim().is_empty() {
            return Ok(token.clone());
        }
    }

    if selection.refresh_token.trim().is_empty() {
        return Err("Prism account is missing msa.refresh_token (log into Prism again)".into());
    }
    if selection.msa_client_id.trim().is_empty() {
        return Err("Prism account is missing msa-client-id".into());
    }

    let ms_access_token =
        refresh_microsoft_access_token_v2(&selection.msa_client_id, &selection.refresh_token)
            .await?;
    let http_client = reqwest::Client::new();
    let mc_flow = MinecraftAuthorizationFlow::new(http_client.clone());
    let mc_token = mc_flow.exchange_microsoft_token(&ms_access_token).await?;
    let minecraft_access_token = mc_token.access_token().as_ref().to_string();

    // Validate token at least maps to a profile.
    let profile = http_client
        .get(MINECRAFT_PROFILE_URL)
        .bearer_auth(&minecraft_access_token)
        .send()
        .await?
        .error_for_status()?
        .json::<MinecraftProfileResponse>()
        .await?;
    if profile.id.trim().is_empty() || profile.name.trim().is_empty() {
        return Err("minecraft profile response missing id/name".into());
    }

    Ok(minecraft_access_token)
}

async fn refresh_microsoft_access_token_v2(
    client_id: &str,
    refresh_token: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let http = reqwest::Client::new();
    let res = http
        .post(MSA_TOKEN_URL_V2)
        .form(&[
            ("client_id", client_id),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("scope", MSA_SCOPE),
        ])
        .send()
        .await?;
    let status = res.status();
    if !status.is_success() {
        let body = res.text().await.unwrap_or_default();
        return Err(format!(
            "Microsoft refresh token request failed (status={} body={})",
            status, body
        )
        .into());
    }
    let token = res.json::<MicrosoftTokenResponseV2>().await?;
    if token.access_token.trim().is_empty() {
        return Err("Microsoft token response missing access_token".into());
    }
    Ok(token.access_token)
}

fn select_prism_account(
    prism_path: &str,
    selected_uuid: Option<&str>,
) -> Result<PrismAuthSelection, Box<dyn std::error::Error>> {
    let raw = std::fs::read_to_string(prism_path)?;
    let root: Value = serde_json::from_str(&raw)?;
    let accounts = root
        .get("accounts")
        .and_then(Value::as_array)
        .ok_or("Prism accounts.json missing `accounts` array")?;
    if accounts.is_empty() {
        return Err("Prism accounts.json has no accounts".into());
    }

    fn normalize_uuid(s: &str) -> String {
        let mut out = s.to_string();
        out.retain(|c| c != '-');
        out
    }
    let selected_uuid = selected_uuid.map(normalize_uuid);

    let mut best: Option<&Value> = None;
    for acc in accounts {
        if acc.get("type").and_then(Value::as_str) != Some("MSA") {
            continue;
        }
        let uuid = acc
            .pointer("/profile/id")
            .and_then(Value::as_str)
            .unwrap_or("");
        let uuid = normalize_uuid(uuid);
        if uuid.len() != 32 {
            continue;
        }

        if let Some(wanted) = &selected_uuid {
            if uuid.eq_ignore_ascii_case(wanted) {
                best = Some(acc);
                break;
            }
        }
        // Default to the first Prism account in the file (predictable and matches UI default).
        if best.is_none() {
            best = Some(acc);
        }
    }

    let acc = best.ok_or("No MSA account found in Prism accounts.json")?;
    let username = acc
        .pointer("/profile/name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let uuid_raw = acc
        .pointer("/profile/id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let uuid = normalize_uuid(uuid_raw);
    let refresh_token = acc
        .pointer("/msa/refresh_token")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let msa_client_id = acc
        .get("msa-client-id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let ygg_token = acc
        .pointer("/ygg/token")
        .and_then(Value::as_str)
        .map(|s| s.to_string());
    let ygg_exp = acc.pointer("/ygg/exp").and_then(Value::as_i64);

    if username.trim().is_empty() || uuid.len() != 32 {
        return Err("Selected Prism account is missing profile.name/profile.id".into());
    }

    Ok(PrismAuthSelection {
        username,
        uuid,
        msa_client_id,
        refresh_token,
        ygg_token,
        ygg_exp,
    })
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
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
        "Server requires online-mode authentication; use Authenticated mode with Prism auth"
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
