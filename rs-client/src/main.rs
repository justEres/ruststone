use bevy::asset::AssetPlugin;
use bevy::window::PresentMode;
use bevy::{log::LogPlugin, prelude::*};
use clap::{Parser, ValueEnum};
use rs_render::RenderPlugin;
use rs_ui::{ConnectUiState, UiPlugin};
use rs_utils::{ApplicationState, AuthMode, FromNet, RUSTSTONE_ASSETS_ROOT_ENV, ToNet};
use rs_utils::{FromNetMessage, ToNetMessage};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

mod embedded_assets;
mod entities;
mod entity_model;
mod inventory_systems;
mod item_textures;
mod message_handler;
mod net;
mod plugins;
mod sim;
mod sim_systems;
mod timing;

use plugins::{
    ClientCorePlugin, ClientEntityPlugin, ClientInventoryPlugin, ClientItemTexturePlugin,
    ClientNetPlugin, ClientSimPlugin, ClientTimingPlugin,
};

const DEFAULT_DEBUG_USERNAME: &str = "RustyPlayer";
const DEFAULT_DEBUG_ADDRESS: &str = "localhost:25565";

#[derive(ValueEnum, Debug, Clone, Copy)]
enum CliAuthMode {
    Offline,
    Authenticated,
}

impl From<CliAuthMode> for AuthMode {
    fn from(value: CliAuthMode) -> Self {
        match value {
            CliAuthMode::Offline => AuthMode::Offline,
            CliAuthMode::Authenticated => AuthMode::Authenticated,
        }
    }
}

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "Ruststone client", long_about = None)]
struct Cli {
    /// Auto-connect on startup.
    #[arg(long, default_value_t = false)]
    autoconnect: bool,
    /// Server address (host:port) used for startup defaults and autoconnect.
    #[arg(long, default_value = DEFAULT_DEBUG_ADDRESS)]
    address: String,
    /// Username used for startup defaults and autoconnect.
    #[arg(long, default_value = DEFAULT_DEBUG_USERNAME)]
    username: String,
    /// Authentication mode for connect requests.
    #[arg(long, value_enum, default_value_t = CliAuthMode::Authenticated)]
    auth_mode: CliAuthMode,
}

fn main() {
    let cli = Cli::parse();
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,wgpu_hal::vulkan=error"));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .without_time()
        .compact()
        .init();

    info!("Starting ruststone");
    let assets_root = match embedded_assets::resolve_runtime_assets_root() {
        Ok(path) => path,
        Err(err) => {
            warn!("failed to resolve runtime assets root: {}", err);
            rs_utils::ruststone_assets_root()
        }
    };
    // Safe because this runs before spawning worker threads.
    unsafe {
        std::env::set_var(RUSTSTONE_ASSETS_ROOT_ENV, assets_root.as_os_str());
    }

    let thread_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    let (tx_outgoing, rx_outgoing) = crossbeam::channel::unbounded::<ToNetMessage>();
    let (tx_incoming, rx_incoming) = crossbeam::channel::unbounded::<FromNetMessage>();

    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024) // 16 MB
        .name("networking".into())
        .spawn(move || {
            rs_net::start_networking(rx_outgoing, tx_incoming);
        })
        .expect("Failed to spawn networking thread");

    if cli.autoconnect {
        let _ = tx_outgoing.send(ToNetMessage::Connect {
            username: cli.username.clone(),
            address: cli.address.clone(),
            auth_mode: cli.auth_mode.into(),
            auth_account_uuid: None,
            prism_accounts_path: None,
        });
    }

    let initial_state = if cli.autoconnect {
        ApplicationState::Connecting
    } else {
        ApplicationState::Disconnected
    };

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(bevy::app::TaskPoolPlugin {
                    task_pool_options: bevy::app::TaskPoolOptions::with_num_threads(thread_count),
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Ruststone Client".into(),
                        resolution: (1080., 720.).into(),
                        resizable: true,
                        present_mode: PresentMode::AutoNoVsync,
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .set(AssetPlugin {
                    file_path: assets_root.to_string_lossy().into_owned(),
                    ..Default::default()
                })
                .build()
                .disable::<LogPlugin>(),
        )
        .add_plugins(RenderPlugin)
        .add_plugins(ClientTimingPlugin)
        .add_plugins(ClientCorePlugin::new(
            initial_state,
            ToNet(tx_outgoing),
            FromNet(rx_incoming),
            ConnectUiState {
                username: cli.username.clone(),
                server_address: cli.address.clone(),
                auth_mode: cli.auth_mode.into(),
                ..Default::default()
            },
        ))
        .add_plugins(UiPlugin)
        .add_plugins(ClientNetPlugin)
        .add_plugins(ClientInventoryPlugin)
        .add_plugins(ClientItemTexturePlugin)
        .add_plugins(ClientSimPlugin)
        .add_plugins(ClientEntityPlugin)
        .run();
}
