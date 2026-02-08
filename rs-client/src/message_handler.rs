use bevy::ecs::system::ResMut;
use bevy::prelude::*;
use rs_render::{ChunkUpdateQueue, LookAngles, Player, PlayerCamera};
use rs_utils::{AppState, ApplicationState, Chat, FromNet, FromNetMessage};

pub fn handle_messages(
    from_net: ResMut<FromNet>,
    mut app_state: ResMut<AppState>,
    mut chat: ResMut<Chat>,
    mut chunk_updates: ResMut<ChunkUpdateQueue>,
    mut player_query: Query<(&mut Transform, &mut LookAngles), With<Player>>,
    mut camera_query: Query<&mut Transform, (With<PlayerCamera>, Without<Player>)>,
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
            FromNetMessage::PlayerPosition(pos) => {
                if let Ok((mut player_transform, mut look)) = player_query.get_single_mut() {
                    player_transform.translation = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
                    if let (Some(yaw), Some(pitch)) = (pos.yaw, pos.pitch) {
                        look.yaw = yaw.to_radians();
                        look.pitch = pitch.to_radians();
                        player_transform.rotation = Quat::from_axis_angle(Vec3::Y, look.yaw);
                        if let Ok(mut camera_transform) = camera_query.get_single_mut() {
                            camera_transform.rotation = Quat::from_axis_angle(Vec3::X, look.pitch);
                        }
                    }
                }
            }
            _ => { /* Ignore other messages for now */ }
        }
    }
}
