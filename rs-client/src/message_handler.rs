use std::time::Instant;

use bevy::ecs::system::ResMut;
use bevy::prelude::*;
use rs_render::{ChunkUpdateQueue, WorldUpdate};
use rs_utils::{
    AppState, ApplicationState, Chat, FromNet, FromNetMessage, InventoryMessage, InventoryState,
    PerfTimings, PlayerStatus,
};

use crate::entities::{RemoteEntityEventQueue, RemoteEntityRegistry};
use crate::net::events::{NetEvent, NetEventQueue};
use crate::sim::collision::WorldCollisionMap;
use crate::sim::{SimClock, SimReady, SimRenderState, SimState};
use crate::sim_systems::PredictionHistory;
use crate::timing::Timing;

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
    mut remote_entity_events: ResMut<RemoteEntityEventQueue>,
    remote_entity_registry: Res<RemoteEntityRegistry>,
    mut collision_map: ResMut<WorldCollisionMap>,
    mut player_status: ResMut<PlayerStatus>,
    mut timings: ResMut<PerfTimings>,
    sim_state: Res<SimState>,
    mut sim_render: ResMut<SimRenderState>,
    mut sim_clock: ResMut<SimClock>,
    mut sim_ready: ResMut<SimReady>,
    mut history: ResMut<PredictionHistory>,
    mut inventory_state: ResMut<InventoryState>,
) {
    let timer = Timing::start();
    while let Ok(msg) = from_net.0.try_recv() {
        match msg {
            FromNetMessage::Connected => {
                *app_state = AppState(ApplicationState::Connected);
                player_status.dead = false;
                player_status.gamemode = 0;
                player_status.can_fly = false;
                player_status.flying = false;
                player_status.flying_speed = 0.05;
                player_status.walking_speed = 0.1;
                sim_clock.tick = 0;
                sim_ready.0 = false;
                history.0 = PredictionHistory::default().0;
                sim_render.previous = sim_state.current;
                inventory_state.reset();
                println!("Connected to server");
            }
            FromNetMessage::Disconnected => {
                *app_state = AppState(ApplicationState::Disconnected);
                sim_ready.0 = false;
                sim_render.previous = sim_state.current;
                inventory_state.reset();
                player_status.gamemode = 0;
                player_status.can_fly = false;
                player_status.flying = false;
            }
            FromNetMessage::ChatMessage(msg) => {
                chat.0.push_back(msg);
                chat.0.truncate(100); // Keep only the last 100 messages
            }
            FromNetMessage::ChunkData(chunk) => {
                collision_map.update_chunk(chunk.clone());
                chunk_updates.0.push(WorldUpdate::ChunkData(chunk));
            }
            FromNetMessage::BlockUpdates(updates) => {
                for update in updates {
                    collision_map.apply_block_update(update);
                    chunk_updates.0.push(WorldUpdate::BlockUpdate(update));
                }
            }
            FromNetMessage::UpdateHealth {
                health,
                food,
                food_saturation,
            } => {
                let was_dead = player_status.dead;
                player_status.health = health;
                player_status.food = food;
                player_status.food_saturation = food_saturation;
                player_status.dead = health <= 0.0;
                // Respawn transition: reset prediction and wait for authoritative position packet.
                if was_dead && !player_status.dead {
                    sim_clock.tick = 0;
                    sim_ready.0 = false;
                    history.0 = PredictionHistory::default().0;
                    sim_render.previous = sim_state.current;
                }
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

                let on_ground = pos.on_ground.unwrap_or(sim_state.current.on_ground);
                net_events.push(NetEvent::ServerPosLook {
                    pos: position,
                    yaw,
                    pitch,
                    on_ground,
                    recv_instant: Instant::now(),
                });
            }
            FromNetMessage::NetEntity(event) => {
                if let rs_utils::NetEntityMessage::Velocity {
                    entity_id,
                    velocity,
                } = &event
                    && remote_entity_registry.local_entity_id == Some(*entity_id)
                {
                    net_events.push(NetEvent::ServerVelocity {
                        velocity: *velocity,
                    });
                }
                remote_entity_events.push(event);
            }
            FromNetMessage::UpdateExperience {
                experience_bar,
                level,
                total_experience,
            } => {
                player_status.experience_bar = experience_bar.clamp(0.0, 1.0);
                player_status.level = level.max(0);
                player_status.total_experience = total_experience.max(0);
            }
            FromNetMessage::GameMode { gamemode } => {
                // 1.8 join packet: lower 3 bits hold game mode, bit 3 is hardcore flag.
                let mode = gamemode & 0x07;
                player_status.gamemode = mode;
                let can_fly = matches!(mode, 1 | 3);
                player_status.can_fly = can_fly;
                if !can_fly {
                    player_status.flying = false;
                }
            }
            FromNetMessage::PlayerAbilities {
                flags,
                flying_speed,
                walking_speed,
            } => {
                // 1.8 abilities flags: 0x01 invuln, 0x02 flying, 0x04 mayfly, 0x08 creative.
                let can_fly = (flags & 0x04) != 0 || (flags & 0x08) != 0;
                player_status.can_fly = can_fly;
                player_status.flying = (flags & 0x02) != 0 && can_fly;
                player_status.flying_speed = flying_speed;
                player_status.walking_speed = walking_speed;
                if (flags & 0x08) != 0 {
                    player_status.gamemode = 1;
                }
            }
            FromNetMessage::Inventory(event) => {
                apply_inventory_message(&mut inventory_state, event);
            }
            _ => { /* Ignore other messages for now */ }
        }
    }
    timings.handle_messages_ms = timer.ms();
}

fn apply_inventory_message(inventory_state: &mut InventoryState, event: InventoryMessage) {
    match event {
        InventoryMessage::WindowOpen(open) => {
            inventory_state.open_window = Some(open.clone());
            inventory_state
                .window_slots
                .entry(open.id)
                .or_insert_with(|| vec![None; open.slot_count as usize]);
        }
        InventoryMessage::WindowClose { id } => {
            if inventory_state
                .open_window
                .as_ref()
                .is_some_and(|window| window.id == id)
            {
                inventory_state.open_window = None;
            }
        }
        InventoryMessage::WindowItems { id, items } => {
            inventory_state.set_window_items(id, items);
        }
        InventoryMessage::WindowSetSlot { id, slot, item } => {
            inventory_state.set_slot(id, slot, item);
        }
        InventoryMessage::ConfirmTransaction {
            id,
            action_number,
            accepted,
        } => {
            if !accepted {
                inventory_state.queue_confirm_ack(id, action_number);
            }
            let next = action_number.saturating_add(1);
            inventory_state.next_action_number = next.max(0) as u16;
        }
        InventoryMessage::SetCurrentHotbarSlot { slot } => {
            inventory_state.selected_hotbar_slot = slot.min(8);
        }
    }
}
