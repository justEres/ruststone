use bevy::ecs::system::ResMut;
use rs_render::ChunkUpdateQueue;
use rs_utils::{AppState, ApplicationState, Chat, FromNet, FromNetMessage};

pub fn handle_messages(
    from_net: ResMut<FromNet>,
    mut app_state: ResMut<AppState>,
    mut chat: ResMut<Chat>,
    mut chunk_updates: ResMut<ChunkUpdateQueue>,
) {
    while let Ok(msg) = from_net.0.try_recv() {
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
            FromNetMessage::ChunkData(chunk) => {
                chunk_updates.0.push(chunk);
            }
            _ => { /* Ignore other messages for now */ }
        }
    }
}
