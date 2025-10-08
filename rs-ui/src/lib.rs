use bevy::app::Plugin;
use bevy::prelude::*;
use bevy_egui::{
    EguiContexts, EguiPlugin, EguiPrimaryContextPass,
    egui::{self, TextEdit},
};
use rs_utils::{AppState, ApplicationState, Chat, ToNet, ToNetMessage};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(EguiPrimaryContextPass, connect_ui)
            .add_plugins(EguiPlugin::default())
            .init_resource::<ConnectUiState>();
    }
}

fn connect_ui(
    mut contexts: EguiContexts,
    mut state: ResMut<ConnectUiState>,
    app_state: Res<AppState>,
    to_net: Res<ToNet>,
    mut chat: ResMut<Chat>,
) {
    let show_connect_window;
    //println!("App state: {:?}", app_state.0);
    match app_state.0 {
        ApplicationState::Disconnected | ApplicationState::Connecting => {
            show_connect_window = true;
            //println!("Showing connect window");
        }
        ApplicationState::Connected => {
            show_connect_window = false;
            //println!("Connected, not showing connect window");
        }
    }

    if show_connect_window {
        egui::Window::new("Connect to Server").show(contexts.ctx_mut().unwrap(), |ui| {
            ui.label("Server Address:");
            ui.text_edit_singleline(&mut state.server_address);
            ui.label("Username:");
            ui.text_edit_singleline(&mut state.username);
            if ui.button("Connect").clicked() {
                to_net
                    .0
                    .send(ToNetMessage::Connect {
                        username: state.username.clone(),
                        address: state.server_address.clone(),
                    })
                    .unwrap();
            }
            if let ApplicationState::Connecting = app_state.0 {
                ui.label("Connecting...");
            }
        });
    }

    egui::Window::new("Chat")
        .vscroll(true)
        .show(contexts.ctx_mut().unwrap(), |ui| {
            for msg in chat.0.iter() {
                ui.label(msg);
            }

            let response = ui.text_edit_singleline(&mut chat.1);

            if response.lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                && !chat.1.is_empty()
            {
                to_net
                    .0
                    .send(ToNetMessage::ChatMessage(chat.1.clone()))
                    .unwrap();
                chat.1.clear();
            }
        });
}

#[derive(Resource)]
pub struct ConnectUiState {
    pub username: String,
    pub server_address: String,
}
impl Default for ConnectUiState {
    fn default() -> Self {
        Self {
            username: "RustPlayer".to_string(),
            server_address: "localhost:25565".to_string(),
        }
    }
}
