use std::time::Instant;

use bevy::ecs::system::ResMut;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use rs_render::{ChunkUpdateQueue, WorldUpdate};
use rs_ui::{ChatAutocompleteState, ConnectUiState};
use rs_utils::{
    AppState, ApplicationState, Chat, FromNet, FromNetMessage, InventoryMessage, InventoryState,
    PlayerStatus, ScoreboardMessage, ScoreboardState, SoundCategory, SoundEvent, SoundEventQueue,
    TabListHeaderFooter, TitleMessage, TitleOverlayState, WorldTime,
};
use tracing::{debug, info};

use crate::entities::{RemoteEntityEventQueue, RemoteEntityRegistry};
use crate::movement_session::MovementSession;
use crate::net::events::{NetEvent, NetEventQueue};
use crate::sim::collision::WorldCollisionMap;
use crate::sim::movement::WorldCollision;
use crate::sim::{SimClock, SimReady, SimRenderState, SimState};
use crate::sim_systems::PredictionHistory;
use crate::timing::Timing;

const FLAG_REL_X: u8 = 0x01;
const FLAG_REL_Y: u8 = 0x02;
const FLAG_REL_Z: u8 = 0x04;
const FLAG_REL_YAW: u8 = 0x08;
const FLAG_REL_PITCH: u8 = 0x10;

#[derive(SystemParam)]
pub(crate) struct MessageUiState<'w, 's> {
    app_state: ResMut<'w, AppState>,
    connect_ui: ResMut<'w, ConnectUiState>,
    chat: ResMut<'w, Chat>,
    chat_autocomplete: ResMut<'w, ChatAutocompleteState>,
    inventory_state: ResMut<'w, InventoryState>,
    _marker: std::marker::PhantomData<&'s ()>,
}

#[derive(SystemParam)]
pub(crate) struct GameplayState<'w, 's> {
    player_status: ResMut<'w, PlayerStatus>,
    world_time: ResMut<'w, WorldTime>,
    title_overlay: ResMut<'w, TitleOverlayState>,
    tab_list_header_footer: ResMut<'w, TabListHeaderFooter>,
    scoreboard: ResMut<'w, ScoreboardState>,
    sound_queue: ResMut<'w, SoundEventQueue>,
    sim_render: ResMut<'w, SimRenderState>,
    sim_clock: ResMut<'w, SimClock>,
    sim_ready: ResMut<'w, SimReady>,
    history: ResMut<'w, PredictionHistory>,
    _marker: std::marker::PhantomData<&'s ()>,
}

pub fn handle_messages(
    from_net: ResMut<FromNet>,
    mut ui: MessageUiState,
    mut chunk_updates: ResMut<ChunkUpdateQueue>,
    mut net_events: ResMut<NetEventQueue>,
    mut movement_session: ResMut<MovementSession>,
    mut remote_entity_events: ResMut<RemoteEntityEventQueue>,
    remote_entity_registry: Res<RemoteEntityRegistry>,
    mut collision_map: ResMut<WorldCollisionMap>,
    mut game: GameplayState,
    sim_state: Res<SimState>,
) {
    let timer = Timing::start();
    while let Ok(msg) = from_net.0.try_recv() {
        match msg {
            FromNetMessage::Connected => {
                *ui.app_state = AppState(ApplicationState::Connected);
                ui.connect_ui.connect_feedback.clear();
                ui.chat_autocomplete.clear();
                game.player_status.dead = false;
                game.player_status.gamemode = 0;
                game.player_status.can_fly = false;
                game.player_status.flying = false;
                game.player_status.flying_speed = 0.05;
                game.player_status.walking_speed = 0.1;
                game.player_status.speed_effect_amplifier = None;
                game.player_status.jump_boost_amplifier = None;
                game.world_time.world_age = 0;
                game.world_time.time_of_day = 0;
                game.world_time.last_sync_instant = None;
                game.title_overlay.reset();
                game.tab_list_header_footer.header.clear();
                game.tab_list_header_footer.footer.clear();
                game.scoreboard.reset();
                game.sim_clock.tick = 0;
                game.sim_ready.0 = false;
                game.history.0 = PredictionHistory::default().0;
                game.sim_render.previous = sim_state.current;
                movement_session.reset_all();
                ui.inventory_state.reset();
                info!("Connected to server");
            }
            FromNetMessage::Disconnected => {
                *ui.app_state = AppState(ApplicationState::Disconnected);
                ui.chat_autocomplete.clear();
                game.title_overlay.reset();
                game.tab_list_header_footer.header.clear();
                game.tab_list_header_footer.footer.clear();
                game.scoreboard.reset();
                game.sim_ready.0 = false;
                game.sim_render.previous = sim_state.current;
                movement_session.reset_all();
                ui.inventory_state.reset();
                game.player_status.gamemode = 0;
                game.player_status.can_fly = false;
                game.player_status.flying = false;
                game.player_status.speed_effect_amplifier = None;
                game.player_status.jump_boost_amplifier = None;
            }
            FromNetMessage::DisconnectReason(reason) => {
                ui.connect_ui.connect_feedback = reason.clone();
                ui.chat.0.push_back(format!("Disconnected: {reason}"));
                ui.chat.0.truncate(100);
                *ui.app_state = AppState(ApplicationState::Disconnected);
                game.title_overlay.reset();
                game.tab_list_header_footer.header.clear();
                game.tab_list_header_footer.footer.clear();
                game.scoreboard.reset();
                game.sim_ready.0 = false;
                movement_session.reset_all();
                ui.inventory_state.reset();
            }
            FromNetMessage::ChatMessage(msg) => {
                ui.chat.0.push_back(msg);
                ui.chat.0.truncate(100); // Keep only the last 100 messages
            }
            FromNetMessage::TabCompleteReply(matches) => {
                let Some(pending_query) = ui.chat_autocomplete.pending_query.take() else {
                    continue;
                };
                if ui.chat.1 != pending_query {
                    continue;
                }
                let mut unique = std::collections::HashSet::new();
                ui.chat_autocomplete.suggestions = matches
                    .into_iter()
                    .map(|entry| entry.trim().to_string())
                    .filter(|entry| !entry.is_empty())
                    .filter(|entry| unique.insert(entry.clone()))
                    .collect();
                ui.chat_autocomplete.selected = 0;
                ui.chat_autocomplete.query_snapshot = ui.chat.1.clone();
                ui.chat_autocomplete.suppress_next_clear = false;
            }
            FromNetMessage::Respawn => {
                game.sim_clock.tick = 0;
                game.sim_ready.0 = false;
                game.history.0 = PredictionHistory::default().0;
                game.sim_render.previous = sim_state.current;
                movement_session.reset_all();
                net_events.events.clear();
                collision_map.clear();
                chunk_updates.0.clear();
                chunk_updates.0.push(WorldUpdate::Reset);
            }
            FromNetMessage::ChunkData(chunk) => {
                collision_map.update_chunk(chunk.clone());
                chunk_updates.0.push(WorldUpdate::ChunkData(chunk));
            }
            FromNetMessage::ChunkUnload { x, z } => {
                collision_map.remove_chunk(x, z);
                chunk_updates.0.push(WorldUpdate::UnloadChunk(x, z));
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
                let was_dead = game.player_status.dead;
                let previous_health = game.player_status.health;
                game.player_status.health = health;
                game.player_status.food = food;
                game.player_status.food_saturation = food_saturation;
                game.player_status.dead = health <= 0.0;
                // Respawn transition: reset prediction and wait for authoritative position packet.
                if was_dead && !game.player_status.dead {
                    game.sim_clock.tick = 0;
                    game.sim_ready.0 = false;
                    game.history.0 = PredictionHistory::default().0;
                    game.sim_render.previous = sim_state.current;
                }
                if !was_dead && health < previous_health {
                    game.sound_queue.push(SoundEvent::Ui {
                        event_id: "minecraft:game.player.hurt".to_string(),
                        volume: 1.0,
                        pitch: 1.0,
                        category_override: Some(SoundCategory::Player),
                    });
                }
            }
            FromNetMessage::PlayerPosition(pos) => {
                let raw_position = pos.position;
                let raw_yaw = pos.yaw;
                let raw_pitch = pos.pitch;
                let raw_flags = pos.flags;
                let raw_on_ground = pos.on_ground;
                let mut exact_position = (
                    sim_state.current.pos.x as f64,
                    sim_state.current.pos.y as f64,
                    sim_state.current.pos.z as f64,
                );
                let mut position = sim_state.current.pos;
                if let Some((x, y, z)) = pos.position {
                    let flags = pos.flags.unwrap_or(0);
                    if (flags & FLAG_REL_X) != 0 {
                        exact_position.0 += x;
                        position.x += x as f32;
                    } else {
                        exact_position.0 = x;
                        position.x = x as f32;
                    }
                    if (flags & FLAG_REL_Y) != 0 {
                        exact_position.1 += y;
                        position.y += y as f32;
                    } else {
                        exact_position.1 = y;
                        position.y = y as f32;
                    }
                    if (flags & FLAG_REL_Z) != 0 {
                        exact_position.2 += z;
                        position.z += z as f32;
                    } else {
                        exact_position.2 = z;
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

                // 1.8 player position packets commonly omit on_ground.
                // Infer support from local collision instead of carrying the previous flag
                // through teleports/setbacks, which tends to make ground state sticky.
                let inferred_on_ground = WorldCollision::with_map(&collision_map).is_supported(position);
                let on_ground_known = pos.on_ground.is_some();
                let on_ground = pos.on_ground.unwrap_or(inferred_on_ground);
                debug!(
                    "[net/correction] tick={} raw_pos={:?} raw_yaw={:?} raw_pitch={:?} flags={:?} raw_on_ground={:?} -> resolved_pos=({:.4},{:.4},{:.4}) resolved_yaw={:.4}rad resolved_pitch={:.4}rad resolved_on_ground={}",
                    game.sim_clock.tick,
                    raw_position,
                    raw_yaw,
                    raw_pitch,
                    raw_flags,
                    raw_on_ground,
                    position.x,
                    position.y,
                    position.z,
                    yaw,
                    pitch,
                    on_ground
                );
                net_events.push(NetEvent::ServerPosLook {
                    pos: position,
                    ack_pos: exact_position,
                    yaw,
                    pitch,
                    on_ground,
                    on_ground_known,
                    recv_instant: Instant::now(),
                });
            }
            FromNetMessage::NetEntity(event) => {
                if let rs_utils::NetEntityMessage::CollectItem {
                    collector_entity_id,
                    ..
                } = &event
                    && remote_entity_registry.local_entity_id == Some(*collector_entity_id)
                {
                    game.sound_queue.push(SoundEvent::Ui {
                        event_id: "minecraft:random.pop".to_string(),
                        volume: 0.2,
                        pitch: 2.0,
                        category_override: Some(SoundCategory::Player),
                    });
                }
                if let rs_utils::NetEntityMessage::Velocity {
                    entity_id,
                    velocity,
                } = &event
                    && remote_entity_registry.local_entity_id == Some(*entity_id)
                {
                    net_events.push(NetEvent::ServerVelocity {
                        velocity: *velocity,
                        recv_instant: Instant::now(),
                    });
                }
                remote_entity_events.push(event);
            }
            FromNetMessage::UpdateExperience {
                experience_bar,
                level,
                total_experience,
            } => {
                let previous_level = game.player_status.level;
                game.player_status.experience_bar = experience_bar.clamp(0.0, 1.0);
                game.player_status.level = level.max(0);
                game.player_status.total_experience = total_experience.max(0);
                if game.player_status.level > previous_level {
                    game.sound_queue.push(SoundEvent::Ui {
                        event_id: "minecraft:random.levelup".to_string(),
                        volume: 0.75,
                        pitch: 1.0,
                        category_override: Some(SoundCategory::Player),
                    });
                }
            }
            FromNetMessage::GameMode { gamemode } => {
                // 1.8 join packet: lower 3 bits hold game mode, bit 3 is hardcore flag.
                let mode = gamemode & 0x07;
                game.player_status.gamemode = mode;
                let can_fly = matches!(mode, 1 | 3);
                game.player_status.can_fly = can_fly;
                if !can_fly {
                    game.player_status.flying = false;
                }
            }
            FromNetMessage::TimeUpdate {
                world_age,
                time_of_day,
            } => {
                game.world_time.world_age = world_age;
                game.world_time.time_of_day = time_of_day;
                game.world_time.last_sync_instant = Some(Instant::now());
            }
            FromNetMessage::PlayerAbilities {
                flags,
                flying_speed,
                walking_speed,
            } => {
                // 1.8 abilities flags: 0x01 invuln, 0x02 flying, 0x04 mayfly, 0x08 creative.
                // For vanilla-accurate movement/anticheat parity, gate flight by gamemode
                // instead of trusting mayfly from plugins/server-side capability toggles.
                let gm_allows_flight = matches!(game.player_status.gamemode, 1 | 3);
                game.player_status.can_fly = gm_allows_flight;
                game.player_status.flying = (flags & 0x02) != 0 && gm_allows_flight;
                game.player_status.flying_speed = flying_speed;
                game.player_status.walking_speed = walking_speed;
            }
            FromNetMessage::EntityAttributes {
                entity_id,
                movement_speed,
            } => {
                if remote_entity_registry.local_entity_id == Some(entity_id)
                    && let Some(movement_speed) = movement_speed
                {
                    game.player_status.walking_speed = movement_speed.max(0.0);
                }
            }
            FromNetMessage::PotionEffect {
                entity_id,
                effect_id,
                amplifier,
                duration_ticks: _duration_ticks,
            } => {
                if remote_entity_registry.local_entity_id == Some(entity_id) {
                    let amp = amplifier.max(0) as u8;
                    match effect_id {
                        1 => game.player_status.speed_effect_amplifier = Some(amp),
                        8 => game.player_status.jump_boost_amplifier = Some(amp),
                        _ => {}
                    }
                }
            }
            FromNetMessage::PotionEffectRemove {
                entity_id,
                effect_id,
            } => {
                if remote_entity_registry.local_entity_id == Some(entity_id) {
                    match effect_id {
                        1 => game.player_status.speed_effect_amplifier = None,
                        8 => game.player_status.jump_boost_amplifier = None,
                        _ => {}
                    }
                }
            }
            FromNetMessage::Inventory(event) => {
                apply_inventory_message(
                    &mut ui.inventory_state,
                    &mut game.sound_queue,
                    &mut movement_session,
                    event,
                );
            }
            FromNetMessage::Sound(event) => {
                game.sound_queue.push(event);
            }
            FromNetMessage::Title(event) => match event {
                TitleMessage::SetTitle { text } => {
                    game.title_overlay.title = text;
                    game.title_overlay.title_started_at = Some(Instant::now());
                }
                TitleMessage::SetSubtitle { text } => {
                    game.title_overlay.subtitle = text;
                    if game.title_overlay.title_started_at.is_none() {
                        game.title_overlay.title_started_at = Some(Instant::now());
                    }
                }
                TitleMessage::SetActionBar { text } => {
                    game.title_overlay.action_bar = text;
                    game.title_overlay.action_bar_started_at = Some(Instant::now());
                }
                TitleMessage::SetTimes {
                    fade_in_ticks,
                    stay_ticks,
                    fade_out_ticks,
                } => {
                    game.title_overlay.times.fade_in_ticks = fade_in_ticks.max(0);
                    game.title_overlay.times.stay_ticks = stay_ticks.max(0);
                    game.title_overlay.times.fade_out_ticks = fade_out_ticks.max(0);
                }
                TitleMessage::Clear => game.title_overlay.clear(),
                TitleMessage::Reset => game.title_overlay.reset(),
            },
            FromNetMessage::TabListHeaderFooter { header, footer } => {
                game.tab_list_header_footer.header = header;
                game.tab_list_header_footer.footer = footer;
            }
            FromNetMessage::Scoreboard(event) => match event {
                ScoreboardMessage::Display {
                    position,
                    objective_name,
                } => {
                    game.scoreboard.set_display_slot(position, objective_name);
                }
                ScoreboardMessage::Objective {
                    name,
                    mode,
                    display_name,
                    render_type,
                } => match mode.unwrap_or(0) {
                    0 | 2 => game
                        .scoreboard
                        .set_objective(name, display_name, render_type),
                    1 => game.scoreboard.remove_objective(&name),
                    _ => {}
                },
                ScoreboardMessage::UpdateScore {
                    entry_name,
                    action,
                    objective_name,
                    value,
                } => match action {
                    0 => game
                        .scoreboard
                        .set_score(entry_name, objective_name, value.unwrap_or(0)),
                    1 => game.scoreboard.remove_score(&entry_name, &objective_name),
                    _ => {}
                },
                ScoreboardMessage::Team {
                    name,
                    mode,
                    display_name,
                    prefix,
                    suffix,
                    players,
                } => {
                    game.scoreboard
                        .apply_team(name, mode, display_name, prefix, suffix, players);
                }
            },
            _ => { /* Ignore other messages for now */ }
        }
    }
    let _ = timer.ms();
}

fn apply_inventory_message(
    inventory_state: &mut InventoryState,
    sound_queue: &mut SoundEventQueue,
    movement_session: &mut MovementSession,
    event: InventoryMessage,
) {
    match event {
        InventoryMessage::WindowOpen(open) => {
            if open.kind.to_ascii_lowercase().contains("chest")
                || open.title.to_ascii_lowercase().contains("chest")
            {
                sound_queue.push(SoundEvent::Ui {
                    event_id: "minecraft:random.chestopen".to_string(),
                    volume: 0.5,
                    pitch: 1.0,
                    category_override: Some(SoundCategory::Block),
                });
            }
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
                if inventory_state.open_window.as_ref().is_some_and(|window| {
                    window.kind.to_ascii_lowercase().contains("chest")
                        || window.title.to_ascii_lowercase().contains("chest")
                }) {
                    sound_queue.push(SoundEvent::Ui {
                        event_id: "minecraft:random.chestclosed".to_string(),
                        volume: 0.5,
                        pitch: 1.0,
                        category_override: Some(SoundCategory::Block),
                    });
                }
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
            inventory_state.apply_transaction_result(id, action_number, accepted);
            if !accepted {
                movement_session.queue_transaction_ack(id, action_number, true);
            }
            let next = action_number.saturating_add(1);
            inventory_state.next_action_number = next.max(0) as u16;
        }
        InventoryMessage::SetCurrentHotbarSlot { slot } => {
            inventory_state.set_selected_hotbar_slot(slot);
        }
    }
}
