use bevy::ecs::system::ResMut;
use bevy::prelude::*;
use rs_render::{ChunkUpdateQueue, LookAngles, Player, PlayerCamera};
use rs_utils::{AppState, ApplicationState, Chat, FromNet, FromNetMessage};

const FLAG_REL_X: u8 = 0x01;
const FLAG_REL_Y: u8 = 0x02;
const FLAG_REL_Z: u8 = 0x04;
const FLAG_REL_YAW: u8 = 0x08;
const FLAG_REL_PITCH: u8 = 0x10;

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
                    if let Some((x, y, z)) = pos.position {
                        let flags = pos.flags.unwrap_or(0);
                        let mut target = player_transform.translation;
                        if (flags & FLAG_REL_X) != 0 {
                            target.x += x as f32;
                        } else {
                            target.x = x as f32;
                        }
                        if (flags & FLAG_REL_Y) != 0 {
                            target.y += y as f32;
                        } else {
                            target.y = y as f32;
                        }
                        if (flags & FLAG_REL_Z) != 0 {
                            target.z += z as f32;
                        } else {
                            target.z = z as f32;
                        }
                        player_transform.translation = target;
                    }

                    if let (Some(yaw), Some(pitch)) = (pos.yaw, pos.pitch) {
                        let flags = pos.flags.unwrap_or(0);
                        let yaw_radians = -yaw.to_radians();
                        let pitch_radians = -pitch.to_radians();

                        if (flags & FLAG_REL_YAW) != 0 {
                            look.yaw += yaw_radians;
                        } else {
                            look.yaw = yaw_radians;
                        }
                        if (flags & FLAG_REL_PITCH) != 0 {
                            look.pitch += pitch_radians;
                        } else {
                            look.pitch = pitch_radians;
                        }

                        look.pitch = look.pitch.clamp(-1.54, 1.54);
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
