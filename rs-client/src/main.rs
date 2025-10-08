use bevy::{log::LogPlugin, prelude::*};
use rs_render::RenderPlugin;
use rs_ui::UiPlugin;
use rs_utils::{AppState, ApplicationState, Chat, FromNet, ToNet};
use rs_utils::{FromNetMessage, ToNetMessage};
use tracing::info;

mod message_handler;

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
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .build()
                .disable::<LogPlugin>(),
        )
        .add_plugins(RenderPlugin)
        .add_plugins(UiPlugin)
        .insert_resource(ToNet(tx_outgoing))
        .insert_resource(FromNet(rx_incoming))
        .insert_resource(AppState(ApplicationState::Disconnected))
        .insert_resource(Chat::default())
        .add_systems(Update, message_handler::handle_messages)
        .run();
}
