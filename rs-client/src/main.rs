use bevy::time::Fixed;
use bevy::window::PresentMode;
use bevy::{diagnostic::FrameTimeDiagnosticsPlugin, log::LogPlugin, prelude::*};
use bevy_egui::EguiPrimaryContextPass;
use clap::{Parser, ValueEnum};
use rs_render::RenderPlugin;
use rs_ui::{ConnectUiState, UiPlugin};
use rs_utils::{
    AppState, ApplicationState, AuthMode, BreakIndicator, Chat, FromNet, InventoryState,
    PerfTimings, PlayerStatus, ToNet, UiState,
};
use rs_utils::{FromNetMessage, ToNetMessage};
use tracing::info;

mod entities;
mod entity_model;
mod inventory_systems;
mod item_textures;
mod message_handler;
mod net;
mod sim;
mod sim_systems;

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
    tracing_subscriber::fmt().without_time().compact().init();

    info!("Starting ruststone");

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
                .build()
                .disable::<LogPlugin>(),
        )
        .insert_resource(Time::<Fixed>::from_seconds(0.05))
        .add_plugins(RenderPlugin)
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_systems(Startup, item_textures::init_item_sprite_mesh)
        .insert_resource(ConnectUiState {
            username: cli.username.clone(),
            server_address: cli.address.clone(),
            auth_mode: cli.auth_mode.into(),
            ..Default::default()
        })
        .add_plugins(UiPlugin)
        .insert_resource(ToNet(tx_outgoing))
        .insert_resource(FromNet(rx_incoming))
        .insert_resource(AppState(initial_state))
        .insert_resource(Chat::default())
        .insert_resource(UiState::default())
        .insert_resource(InventoryState::default())
        .insert_resource(PlayerStatus::default())
        .insert_resource(BreakIndicator::default())
        .insert_resource(PerfTimings::default())
        .insert_resource(net::events::NetEventQueue::default())
        .insert_resource(entities::RemoteEntityEventQueue::default())
        .insert_resource(entities::RemoteEntityRegistry::default())
        .insert_resource(entities::RemoteSkinDownloader::default())
        .insert_resource(entities::PlayerTextureDebugSettings::default())
        .insert_resource(sim::SimClock::default())
        .insert_resource(sim::CurrentInput::default())
        .insert_resource(sim::SimState::default())
        .insert_resource(sim::SimRenderState::default())
        .insert_resource(sim::VisualCorrectionOffset::default())
        .insert_resource(sim::DebugStats::default())
        .insert_resource(sim::SimReady::default())
        .insert_resource(sim::DebugUiState::default())
        .insert_resource(sim::ZoomState::default())
        .insert_resource(sim::collision::WorldCollisionMap::default())
        .insert_resource(sim_systems::PredictionHistory::default())
        .insert_resource(sim_systems::LatencyEstimate::default())
        .insert_resource(sim_systems::ActionState::default())
        .insert_resource(sim_systems::FrameTimingState::default())
        .insert_resource(sim_systems::EntityHitboxDebug::default())
        .insert_resource(item_textures::ItemTextureCache::default())
        .insert_resource(entity_model::EntityTextureCache::default())
        .add_systems(First, sim_systems::frame_timing_start)
        .add_systems(
            Update,
            sim_systems::update_timing_start.before(sim_systems::debug_toggle_system),
        )
        .add_systems(Update, message_handler::handle_messages)
        .add_systems(
            Update,
            (
                entities::remote_entity_connection_sync.after(message_handler::handle_messages),
                entities::apply_remote_entity_events.after(entities::remote_entity_connection_sync),
                entities::remote_skin_download_tick.after(entities::apply_remote_entity_events),
                entities::apply_remote_player_skins.after(entities::remote_skin_download_tick),
                entities::rebuild_remote_player_meshes_on_texture_debug_change
                    .after(entities::apply_remote_player_skins),
                entities::smooth_remote_entity_motion
                    .after(entities::apply_remote_entity_events)
                    .after(entities::rebuild_remote_player_meshes_on_texture_debug_change),
                entities::animate_remote_player_models.after(entities::smooth_remote_entity_motion),
                entities::animate_remote_biped_models.after(entities::smooth_remote_entity_motion),
                entities::billboard_item_sprites.after(entities::smooth_remote_entity_motion),
            ),
        )
        .add_systems(
            Update,
            (
                sim_systems::debug_toggle_system,
                inventory_systems::hotbar_input_system,
                inventory_systems::inventory_transaction_ack_system,
                sim_systems::input_collect_system,
                sim_systems::camera_zoom_system
                    .after(rs_render::debug::apply_render_debug_settings),
                item_textures::item_texture_cache_tick,
                entity_model::entity_texture_cache_tick,
                entities::apply_held_item_visibility_system,
                entities::apply_item_sprite_textures_system
                    .after(item_textures::item_texture_cache_tick),
                entities::apply_entity_model_textures_system
                    .after(entity_model::entity_texture_cache_tick),
                sim_systems::visual_smoothing_system,
                sim_systems::apply_visual_transform_system,
                entities::spawn_local_player_model_system
                    .after(sim_systems::apply_visual_transform_system),
                entities::update_local_player_skin_system
                    .after(entities::spawn_local_player_model_system),
                entities::animate_local_player_model_system
                    .after(sim_systems::apply_visual_transform_system),
                sim_systems::local_held_item_view_system
                    .after(sim_systems::apply_visual_transform_system),
                sim_systems::draw_entity_hitboxes_system
                    .after(sim_systems::apply_visual_transform_system),
                sim_systems::draw_chunk_debug_system
                    .after(sim_systems::apply_visual_transform_system),
                sim_systems::world_interaction_system
                    .after(sim_systems::apply_visual_transform_system),
            ),
        )
        .add_systems(
            Update,
            sim_systems::update_timing_end.after(sim_systems::apply_visual_transform_system),
        )
        .add_systems(
            PostUpdate,
            sim_systems::post_update_timing_start.before(sim_systems::post_update_timing_end),
        )
        .add_systems(
            PostUpdate,
            sim_systems::post_update_timing_end.after(sim_systems::post_update_timing_start),
        )
        .add_systems(
            FixedUpdate,
            (
                sim_systems::net_event_apply_system,
                sim_systems::fixed_sim_tick_system,
            )
                .chain(),
        )
        .add_systems(
            FixedUpdate,
            sim_systems::fixed_update_timing_start.before(sim_systems::net_event_apply_system),
        )
        .add_systems(
            FixedUpdate,
            sim_systems::fixed_update_timing_end.after(sim_systems::fixed_sim_tick_system),
        )
        .add_systems(EguiPrimaryContextPass, sim_systems::debug_overlay_system)
        .add_systems(EguiPrimaryContextPass, entities::draw_remote_entity_names)
        .add_systems(Last, sim_systems::frame_timing_end)
        .run();
}
