use bevy::window::PresentMode;
use bevy::{diagnostic::FrameTimeDiagnosticsPlugin, log::LogPlugin, prelude::*};
use bevy_egui::EguiPrimaryContextPass;
use bevy::time::Fixed;
use rs_render::RenderPlugin;
use rs_ui::UiPlugin;
use rs_utils::{AppState, ApplicationState, Chat, FromNet, PerfTimings, PlayerStatus, ToNet, UiState};
use rs_utils::{FromNetMessage, ToNetMessage};
use tracing::info;

mod message_handler;
mod net;
mod sim;
mod sim_systems;

fn main() {
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
        .add_plugins(UiPlugin)
        .insert_resource(ToNet(tx_outgoing))
        .insert_resource(FromNet(rx_incoming))
        .insert_resource(AppState(ApplicationState::Disconnected))
        .insert_resource(Chat::default())
        .insert_resource(UiState::default())
        .insert_resource(PlayerStatus::default())
        .insert_resource(PerfTimings::default())
        .insert_resource(net::events::NetEventQueue::default())
        .insert_resource(sim::SimClock::default())
        .insert_resource(sim::CurrentInput::default())
        .insert_resource(sim::SimState::default())
        .insert_resource(sim::SimRenderState::default())
        .insert_resource(sim::VisualCorrectionOffset::default())
        .insert_resource(sim::DebugStats::default())
        .insert_resource(sim::SimReady::default())
        .insert_resource(sim::DebugUiState::default())
        .insert_resource(sim::collision::WorldCollisionMap::default())
        .insert_resource(sim_systems::PredictionHistory::default())
        .insert_resource(sim_systems::LatencyEstimate::default())
        .insert_resource(sim_systems::FrameTimingState::default())
        .add_systems(First, sim_systems::frame_timing_start)
        .add_systems(
            Update,
            sim_systems::update_timing_start.before(sim_systems::debug_toggle_system),
        )
        .add_systems(Update, message_handler::handle_messages)
        .add_systems(
            Update,
            (
                sim_systems::debug_toggle_system,
                sim_systems::input_collect_system,
                sim_systems::visual_smoothing_system,
                sim_systems::apply_visual_transform_system,
            ),
        )
        .add_systems(
            Update,
            sim_systems::update_timing_end.after(sim_systems::apply_visual_transform_system),
        )
        .add_systems(
            PostUpdate,
            sim_systems::post_update_timing_start
                .before(sim_systems::post_update_timing_end),
        )
        .add_systems(
            PostUpdate,
            sim_systems::post_update_timing_end.after(sim_systems::post_update_timing_start),
        )
        .add_systems(
            FixedUpdate,
            (sim_systems::net_event_apply_system, sim_systems::fixed_sim_tick_system).chain(),
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
        .add_systems(Last, sim_systems::frame_timing_end)
        .run();
}
