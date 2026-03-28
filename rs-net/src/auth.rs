use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use minecraft_msa_auth::MinecraftAuthorizationFlow;
use rand::Rng;
use rs_protocol::protocol::login::{Account, AccountType};
use rs_protocol::protocol::packet::Packet;
use rs_protocol::protocol::Conn;
use rs_utils::AuthMode;
use serde::Deserialize;
use serde_json::Value;
use tracing::{debug, error, info};

const MSA_TOKEN_URL_V2: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const MSA_SCOPE: &str = "XboxLive.signin offline_access";
const MINECRAFT_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

pub(super) fn connect(
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
            Ok(pkt) => match pkt {
                Packet::SetInitialCompression(s) => {
                    debug!("RECV: SetInitialCompression (threshold={})", s.threshold.0);
                    conn.set_compression(s.threshold.0);
                }
                Packet::EncryptionRequest(req) => {
                    debug!("RECV: EncryptionRequest");
                    handle_encryption_request(
                        &mut conn,
                        &req.server_id,
                        &req.public_key.data,
                        &req.verify_token.data,
                        online_account.as_ref(),
                    )?;
                }
                Packet::EncryptionRequest_i16(req) => {
                    debug!("RECV: EncryptionRequest_i16");
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
                    info!(
                        "RECV: LoginSuccess_String (uuid={}, username={})",
                        s.uuid, s.username
                    );
                    conn.state = rs_protocol::protocol::State::Play;
                    return Ok(conn);
                }
                Packet::LoginSuccess_UUID(s) => {
                    info!(
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
                    debug!("RECV: {} (full={})", variant, dbg);
                }
            },
            Err(e) => {
                error!("Error reading packet: {}", e);
                return Err(Box::new(e));
            }
        }
    }
}

fn default_prism_accounts_pathbuf() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA")
        && !appdata.trim().is_empty()
    {
        return PathBuf::from(appdata)
            .join("PrismLauncher")
            .join("accounts.json");
    }
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

    info!(
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
    account.verification_tokens = vec![String::new(), String::new(), minecraft_access_token];
    Ok(account)
}

async fn get_minecraft_access_token(
    selection: &PrismAuthSelection,
) -> Result<String, Box<dyn std::error::Error>> {
    if let (Some(token), Some(exp)) = (&selection.ygg_token, selection.ygg_exp)
        && exp > unix_now() + 60
        && !token.trim().is_empty()
    {
        return Ok(token.clone());
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

        if let Some(wanted) = &selected_uuid
            && uuid.eq_ignore_ascii_case(wanted)
        {
            best = Some(acc);
            break;
        }
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
    info!(
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
    info!("Encryption enabled");
    Ok(())
}
