use bevy::app::Plugin;
use bevy::input::ButtonInput;
use bevy::prelude::*;
use bevy_egui::{
    egui::{self},
    EguiContexts, EguiPlugin, EguiPrimaryContextPass,
};
use rs_utils::{AppState, ApplicationState, Chat, PlayerStatus, ToNet, ToNetMessage, UiState};

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
    keys: Res<ButtonInput<KeyCode>>,
    mut ui_state: ResMut<UiState>,
    player_status: Res<PlayerStatus>,
) {
    let ctx = contexts.ctx_mut().unwrap();

    if keys.just_pressed(KeyCode::Escape) && ui_state.chat_open {
        ui_state.chat_open = false;
    } else if keys.just_pressed(KeyCode::KeyT) && !ctx.wants_keyboard_input() {
        ui_state.chat_open = !ui_state.chat_open;
        if ui_state.chat_open {
            chat.1.clear();
        }
    }

    let show_connect_window = matches!(
        app_state.0,
        ApplicationState::Disconnected | ApplicationState::Connecting
    );

    if show_connect_window {
        egui::Window::new("Connect to Server").show(ctx, |ui| {
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

    if ui_state.chat_open {
        egui::Window::new("Chat")
            .vscroll(true)
            .show(ctx, |ui| {
                for msg in chat.0.iter() {
                    ui.label(msg);
                }

                let response = ui.text_edit_singleline(&mut chat.1);
                response.request_focus();

                if response.has_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    && !chat.1.is_empty()
                {
                    to_net
                        .0
                        .send(ToNetMessage::ChatMessage(chat.1.clone()))
                        .unwrap();
                    chat.1.clear();
                    response.request_focus();
                }
            });
    }

    if matches!(app_state.0, ApplicationState::Connected) && player_status.dead {
        egui::Area::new("death_screen".into())
            .fixed_pos(egui::pos2(0.0, 0.0))
            .show(ctx, |ui| {
                let screen = ui.ctx().screen_rect();
                ui.set_min_size(screen.size());
                ui.vertical_centered(|ui| {
                    ui.add_space(screen.height() * 0.3);
                    ui.heading("You Died");
                    ui.add_space(8.0);
                    if ui.button("Respawn").clicked() {
                        let _ = to_net.0.send(ToNetMessage::Respawn);
                    }
                });
            });
    }
}

#[derive(Resource)]
pub struct ConnectUiState {
    pub username: String,
    pub server_address: String,
}
impl Default for ConnectUiState {
    fn default() -> Self {
        Self {
            username: "RustyPlayer".to_string(),
            server_address: "localhost:25565".to_string(),
        }
    }
}
