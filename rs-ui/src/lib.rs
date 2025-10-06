use bevy::app::Plugin;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use rs_utils::{ToNet, ToNetMessage};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(EguiPrimaryContextPass, connect_ui)
            .add_plugins(EguiPlugin::default())
            .init_resource::<ConnectUiState>();
        
    }
}

fn connect_ui(mut contexts: EguiContexts, mut state: ResMut<ConnectUiState>, to_net: Res<ToNet>) {

    if state.show_connect_window {
        egui::Window::new("Connect to Server")
            .show(contexts.ctx_mut().unwrap(), |ui| {
                ui.label("Server Address:");
                ui.text_edit_singleline(&mut state.server_address);
            ui.label("Username:");
            ui.text_edit_singleline(&mut state.username);
            if ui.button("Connect").clicked() {

                to_net.0.send(ToNetMessage::Connect {
                    username: state.username.clone(),
                    address: state.server_address.clone(),
                }).unwrap();

            }
        });
    }

}

#[derive(Resource)]
pub struct ConnectUiState {
    pub username: String,
    pub server_address: String,
    pub show_connect_window: bool,
}
impl Default for ConnectUiState {
    fn default() -> Self {
        Self {
            username: "RustPlayer".to_string(),
            server_address: "localhost:25565".to_string(),
            show_connect_window: true, 
        }
    }
}