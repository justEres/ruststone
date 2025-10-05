use bevy::{log::LogPlugin, prelude::*};
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use rs_net::NetworkingPlugin;
use rs_render::RenderPlugin;
use rs_ui::UiPlugin;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().without_time().compact().init();

    info!("Starting ruststone");

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
        .add_plugins(NetworkingPlugin)
        .run();
}
