use std::time::Instant;

use bevy::ecs::system::ResMut;
use bevy::prelude::*;
use rs_render::ChunkUpdateQueue;
use rs_utils::{AppState, ApplicationState, Chat, FromNet, FromNetMessage, PerfTimings, PlayerStatus};

use crate::net::events::{NetEvent, NetEventQueue};
use crate::sim::collision::WorldCollisionMap;
use crate::sim::{SimClock, SimReady, SimRenderState, SimState, VisualCorrectionOffset};
use crate::sim_systems::PredictionHistory;

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
    mut net_events: ResMut<NetEventQueue>,
    mut collision_map: ResMut<WorldCollisionMap>,
    mut player_status: ResMut<PlayerStatus>,
    mut timings: ResMut<PerfTimings>,
    sim_state: Res<SimState>,
    mut sim_render: ResMut<SimRenderState>,
    mut sim_clock: ResMut<SimClock>,
    mut sim_ready: ResMut<SimReady>,
    mut history: ResMut<PredictionHistory>,
    mut visual_offset: ResMut<VisualCorrectionOffset>,
) {
    let start = std::time::Instant::now();
    while let Ok(msg) = from_net.0.try_recv() {
        match msg {
            FromNetMessage::Connected => {
                *app_state = AppState(ApplicationState::Connected);
                sim_clock.tick = 0;
                sim_ready.0 = false;
                history.0 = PredictionHistory::default().0;
                sim_render.previous = sim_state.current;
                visual_offset.0 = Vec3::ZERO;
                println!("Connected to server");
            }
            FromNetMessage::Disconnected => {
                *app_state = AppState(ApplicationState::Disconnected);
                sim_ready.0 = false;
                sim_render.previous = sim_state.current;
            }
            FromNetMessage::ChatMessage(msg) => {
                chat.0.push_back(msg);
                chat.0.truncate(100); // Keep only the last 100 messages
            }
            FromNetMessage::ChunkData(chunk) => {
                collision_map.update_chunk(chunk.clone());
                chunk_updates.0.push(chunk);
            }
            FromNetMessage::UpdateHealth {
                health,
                food,
                food_saturation,
            } => {
                player_status.health = health;
                player_status.food = food;
                player_status.food_saturation = food_saturation;
                player_status.dead = health <= 0.0;
            }
            FromNetMessage::PlayerPosition(pos) => {
                let mut position = sim_state.current.pos;
                if let Some((x, y, z)) = pos.position {
                    let flags = pos.flags.unwrap_or(0);
                    if (flags & FLAG_REL_X) != 0 {
                        position.x += x as f32;
                    } else {
                        position.x = x as f32;
                    }
                    if (flags & FLAG_REL_Y) != 0 {
                        position.y += y as f32;
                    } else {
                        position.y = y as f32;
                    }
                    if (flags & FLAG_REL_Z) != 0 {
                        position.z += z as f32;
                    } else {
                        position.z = z as f32;
                    }
                }

                let mut yaw = sim_state.current.yaw;
                let mut pitch = sim_state.current.pitch;
                if let (Some(yaw_deg), Some(pitch_deg)) = (pos.yaw, pos.pitch) {
                    let flags = pos.flags.unwrap_or(0);
                    let yaw_rad = yaw_deg.to_radians();
                    let pitch_rad = -pitch_deg.to_radians();
                    if (flags & FLAG_REL_YAW) != 0 {
                        yaw -= yaw_rad;
                    } else {
                        yaw = std::f32::consts::PI - yaw_rad;
                    }
                    if (flags & FLAG_REL_PITCH) != 0 {
                        pitch += pitch_rad;
                    } else {
                        pitch = pitch_rad;
                    }
                }

                let on_ground = pos.on_ground.unwrap_or(false);
                net_events.push(NetEvent::ServerPosLook {
                    pos: position,
                    yaw,
                    pitch,
                    on_ground,
                    recv_instant: Instant::now(),
                });
            }
            _ => { /* Ignore other messages for now */ }
        }
    }
    timings.handle_messages_ms = start.elapsed().as_secs_f32() * 1000.0;
}
