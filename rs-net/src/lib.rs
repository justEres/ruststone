#![recursion_limit = "256"]

use std::path::{Path, PathBuf};
use std::thread;

use minecraft_msa_auth::MinecraftAuthorizationFlow;
use oauth2::basic::BasicClient;
use oauth2::devicecode::StandardDeviceAuthorizationResponse;
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, ClientId, DeviceAuthorizationUrl, RefreshToken, Scope, TokenResponse, TokenUrl,
};
use rand::Rng;
use rs_protocol::protocol::Conn;
use rs_protocol::protocol::login::{Account, AccountType};
use rs_protocol::protocol::packet::Packet;
use rs_utils::{AuthMode, EntityUseAction, FromNetMessage, InventoryItemStack, ToNetMessage};
use serde::{Deserialize, Serialize};

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
            } => {
                return Some(ConnectRequest {
                    username,
                    address,
                    auth_mode,
                    auth_account_uuid,
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
    auth_account_uuid: Option<&str>,
) -> Result<Conn, Box<dyn std::error::Error>> {
    let mut conn = Conn::new(target, 47)?;
    let online_account = match auth_mode {
        AuthMode::Offline => None,
        AuthMode::Authenticated => Some(
            load_online_account(username, auth_account_uuid).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Authenticated mode selected but no valid account could be loaded",
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

const DEFAULT_MSA_CLIENT_ID: &str = "00000000402b5328";
const DEVICE_CODE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const MSA_AUTHORIZE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";
const MSA_TOKEN_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";
const MINECRAFT_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
struct AuthAccountsFile {
    version: u32,
    accounts: Vec<AuthAccountRecord>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
struct AuthAccountRecord {
    username: String,
    uuid: String,
    refresh_token: String,
}

#[derive(Debug)]
struct OnlineAuthData {
    username: String,
    uuid: String,
    minecraft_access_token: String,
    refresh_token: String,
}

#[derive(Debug, Deserialize)]
struct MinecraftProfileResponse {
    id: String,
    name: String,
}

fn load_online_account(preferred_username: &str, selected_uuid: Option<&str>) -> Option<Account> {
    match load_online_account_from_msa(preferred_username, selected_uuid) {
        Ok(account) => Some(account),
        Err(err) => {
            println!("Microsoft authentication failed: {}", err);
            None
        }
    }
}

fn load_online_account_from_msa(
    preferred_username: &str,
    selected_uuid: Option<&str>,
) -> Result<Account, Box<dyn std::error::Error>> {
    let store_path = auth_accounts_path();
    let mut store = read_auth_accounts(&store_path).unwrap_or_default();
    if store.version == 0 {
        store.version = 1;
    }
    let selected_refresh = select_account(&store, selected_uuid)
        .map(|record| record.refresh_token.as_str())
        .filter(|token| !token.trim().is_empty());

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let auth = runtime.block_on(authenticate_with_msa(
        DEFAULT_MSA_CLIENT_ID,
        preferred_username,
        selected_refresh,
    ))?;

    upsert_account(
        &mut store,
        AuthAccountRecord {
            username: auth.username.clone(),
            uuid: auth.uuid.clone(),
            refresh_token: auth.refresh_token.clone(),
        },
    );

    if let Err(err) = write_auth_accounts(&store_path, &store) {
        println!(
            "Failed to write auth account store {}: {}",
            store_path.display(),
            err
        );
    }

    println!(
        "Auth MSA loaded: username={} uuid={}.. token_len={}",
        auth.username,
        &auth.uuid[..8],
        auth.minecraft_access_token.len()
    );
    let mut account = Account::new(auth.username, Some(auth.uuid), AccountType::Microsoft);
    account.verification_tokens = vec![String::new(), String::new(), auth.minecraft_access_token];
    Ok(account)
}

async fn authenticate_with_msa(
    client_id: &str,
    preferred_username: &str,
    refresh_token_hint: Option<&str>,
) -> Result<OnlineAuthData, Box<dyn std::error::Error>> {
    let oauth = BasicClient::new(
        ClientId::new(client_id.to_string()),
        None,
        AuthUrl::new(MSA_AUTHORIZE_URL.to_string())?,
        Some(TokenUrl::new(MSA_TOKEN_URL.to_string())?),
    )
    .set_device_authorization_url(DeviceAuthorizationUrl::new(DEVICE_CODE_URL.to_string())?);

    let mut refresh_token = refresh_token_hint.unwrap_or_default().to_string();

    let token = if !refresh_token.is_empty() {
        match oauth
            .exchange_refresh_token(&RefreshToken::new(refresh_token.clone()))
            .request_async(async_http_client)
            .await
        {
            Ok(token) => {
                println!("Authenticated via cached refresh token");
                token
            }
            Err(err) => {
                println!("Refresh token auth failed ({}), starting device auth", err);
                refresh_token.clear();
                authenticate_with_device_code(&oauth).await?
            }
        }
    } else {
        authenticate_with_device_code(&oauth).await?
    };

    if let Some(new_refresh) = token.refresh_token() {
        refresh_token = new_refresh.secret().to_string();
    }
    if refresh_token.is_empty() {
        return Err("missing refresh token after Microsoft auth".into());
    }

    let ms_access_token = token.access_token().secret().to_string();
    let http_client = reqwest::Client::new();
    let mc_flow = MinecraftAuthorizationFlow::new(http_client.clone());
    let mc_token = mc_flow.exchange_microsoft_token(&ms_access_token).await?;
    let minecraft_access_token = mc_token.access_token().as_ref().to_string();

    let profile = http_client
        .get(MINECRAFT_PROFILE_URL)
        .bearer_auth(&minecraft_access_token)
        .send()
        .await?
        .error_for_status()?
        .json::<MinecraftProfileResponse>()
        .await?;

    let mut uuid = profile.id;
    uuid.retain(|c| c != '-');
    if uuid.len() != 32 {
        return Err("minecraft profile id is invalid".into());
    }

    if !preferred_username.is_empty() && !profile.name.eq_ignore_ascii_case(preferred_username) {
        println!(
            "Using authenticated profile '{}' instead of requested username '{}'",
            profile.name, preferred_username
        );
    }

    Ok(OnlineAuthData {
        username: profile.name,
        uuid,
        minecraft_access_token,
        refresh_token,
    })
}

async fn authenticate_with_device_code(
    oauth: &BasicClient,
) -> Result<
    oauth2::StandardTokenResponse<oauth2::EmptyExtraTokenFields, oauth2::basic::BasicTokenType>,
    Box<dyn std::error::Error>,
> {
    let details: StandardDeviceAuthorizationResponse = oauth
        .exchange_device_code()?
        .add_scope(Scope::new("XboxLive.signin offline_access".to_string()))
        .request_async(async_http_client)
        .await?;

    println!(
        "Open this URL in your browser:\n{}\nEnter code: {}",
        details.verification_uri().to_string(),
        details.user_code().secret()
    );

    let token = oauth
        .exchange_device_access_token(&details)
        .request_async(async_http_client, tokio::time::sleep, None)
        .await?;
    Ok(token)
}

fn auth_accounts_path() -> PathBuf {
    if let Ok(home) = std::env::var("HOME")
        && !home.trim().is_empty()
    {
        return PathBuf::from(home)
            .join(".config")
            .join("ruststone")
            .join("accounts.json");
    }

    PathBuf::from(".ruststone-accounts.json")
}

fn read_auth_accounts(path: &Path) -> Option<AuthAccountsFile> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_auth_accounts(
    path: &Path,
    store: &AuthAccountsFile,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(store)?;
    std::fs::write(path, raw)?;
    Ok(())
}

fn select_account<'a>(
    store: &'a AuthAccountsFile,
    selected_uuid: Option<&str>,
) -> Option<&'a AuthAccountRecord> {
    if store.accounts.is_empty() {
        return None;
    }
    if let Some(uuid) = selected_uuid
        && let Some(found) = store
            .accounts
            .iter()
            .find(|account| account.uuid.eq_ignore_ascii_case(uuid))
    {
        return Some(found);
    }
    store.accounts.first()
}

fn upsert_account(store: &mut AuthAccountsFile, record: AuthAccountRecord) {
    if let Some(existing) = store
        .accounts
        .iter_mut()
        .find(|entry| entry.uuid.eq_ignore_ascii_case(&record.uuid))
    {
        *existing = record;
    } else {
        store.accounts.push(record);
    }
}

fn handle_encryption_request(
    conn: &mut Conn,
    server_id: &str,
    public_key: &[u8],
    verify_token: &[u8],
    online_account: Option<&Account>,
) -> Result<(), Box<dyn std::error::Error>> {
    let account = online_account.ok_or_else(|| {
        "Server requires online-mode authentication; use Authenticated mode to run Microsoft device login"
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
