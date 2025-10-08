use bevy::ecs::system::ResMut;
use rs_utils::{AppState, ApplicationState, Chat, FromNet, FromNetMessage};

pub fn handle_messages(
    from_net: ResMut<FromNet>,
    mut app_state: ResMut<AppState>,
    mut chat: ResMut<Chat>,
) {
    while let Ok(msg) = from_net.0.try_recv() {
        //println!("Handling FromNetMessage");
        match msg {
            FromNetMessage::Connected => {
                *app_state = AppState(ApplicationState::Connected);
                println!("Connected to server");
            }
            FromNetMessage::Disconnected => {
                *app_state = AppState(ApplicationState::Disconnected);
            }
            FromNetMessage::ChatMessage(msg) => {
                chat.0.push_back(msg);
                chat.0.truncate(100); // Keep only the last 100 messages
            }
            _ => { /* Ignore other messages for now */ }
        }
    }
}
