use bevy::ecs::system::ResMut;
use rs_utils::{AppState, ApplicationState, FromNet, FromNetMessage};

pub fn handle_messages(from_net: ResMut<FromNet>, mut app_state: ResMut<AppState>) {
    while let Ok(msg) = from_net.0.try_recv() {
        //println!("Handling FromNetMessage");
        match msg {
            FromNetMessage::Connected => {
                *app_state = AppState(ApplicationState::Connected);
            }
            FromNetMessage::Disconnected => {
                *app_state = AppState(ApplicationState::Disconnected);
            }
            _ => {}
        }
    }
}
