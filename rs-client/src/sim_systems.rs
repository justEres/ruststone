use std::time::Instant;

use bevy::input::mouse::MouseMotion;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::time::Fixed;
use bevy_egui::{EguiContexts, egui};

use rs_render::{ChunkRoot, LookAngles, Player, PlayerCamera};
use rs_utils::{AppState, ApplicationState, ToNet, ToNetMessage, UiState};

use crate::net::events::NetEventQueue;
use crate::sim::collision::WorldCollisionMap;
use crate::sim::movement::{
    WorldCollision, debug_block_collision_boxes, effective_sprint, simulate_tick,
};
use crate::sim::predict::PredictionBuffer;
use crate::sim::{
    CameraPerspectiveAltHold, CameraPerspectiveMode, CameraPerspectiveState, CorrectionLoopGuard,
    CurrentInput, DebugStats, DebugUiState, LocalArmSwing, MovementPacketState, PredictedFrame,
    SimClock, SimRenderState, SimState, VisualCorrectionOffset, ZoomState,
};
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use rs_render::{RenderDebugSettings, debug::RenderPerfStats};
use rs_utils::{
    BreakIndicator, EntityUseAction, InventoryState, PerfTimings, block_model_kind,
    block_registry_key, block_state_id, block_state_meta,
};

use crate::entities::{ItemSpriteStack, PlayerTextureDebugSettings, RemoteVisual};
use crate::entities::{RemoteEntity, RemoteEntityRegistry};
use crate::item_textures::{ItemSpriteMesh, ItemTextureCache};
use crate::timing::Timing;

#[derive(Resource, Default)]
pub struct EntityHitboxDebug {
    pub enabled: bool,
}

#[derive(Resource, Default)]
pub struct FrameTimingState {
    pub start: Option<Instant>,
    pub update_start: Option<Instant>,
    pub post_update_start: Option<Instant>,
    pub fixed_update_start: Option<Instant>,
}

#[derive(Resource)]
pub struct PredictionHistory(pub PredictionBuffer);

impl Default for PredictionHistory {
    fn default() -> Self {
        Self(PredictionBuffer::new(512))
    }
}

#[derive(Default, Resource)]
pub struct LatencyEstimate {
    pub one_way_ticks: u32,
    pub last_sent: Option<Instant>,
}

#[derive(Default, Resource)]
pub struct ActionState {
    pub sneaking: bool,
    pub sprinting: bool,
    pub jump_was_pressed: bool,
    pub fly_toggle_timer: u8,
}

#[derive(Default)]
pub(crate) struct MiningState {
    active: bool,
    target_block: IVec3,
    face: u8,
    elapsed_secs: f32,
    total_secs: f32,
    finish_sent: bool,
}

const SURVIVAL_BLOCK_REACH: f32 = 4.5;
const CREATIVE_BLOCK_REACH: f32 = 5.0;
const SURVIVAL_ENTITY_REACH: f32 = 3.0;
const CREATIVE_ENTITY_REACH: f32 = 5.0;

fn wrap_degrees(mut deg: f32) -> f32 {
    while deg <= -180.0 {
        deg += 360.0;
    }
    while deg > 180.0 {
        deg -= 360.0;
    }
    deg
}

fn client_abilities_flags(player_status: &rs_utils::PlayerStatus) -> u8 {
    let mut flags = 0u8;
    if player_status.flying {
        flags |= 0x02;
    }
    if player_status.can_fly {
        flags |= 0x04;
    }
    if player_status.gamemode == 1 {
        // Creative mode bit (instabuild). Servers commonly pair this with mayfly.
        flags |= 0x08;
    }
    flags
}

pub fn input_collect_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut motion_events: EventReader<MouseMotion>,
    mut input: ResMut<CurrentInput>,
    perspective: Res<CameraPerspectiveState>,
    app_state: Res<AppState>,
    ui_state: Res<UiState>,
    player_status: Res<rs_utils::PlayerStatus>,
    mut timings: ResMut<PerfTimings>,
) {
    let timer = Timing::start();
    if !matches!(app_state.0, ApplicationState::Connected)
        || ui_state.chat_open
        || ui_state.paused
        || ui_state.inventory_open
        || player_status.dead
    {
        motion_events.clear();
        input.0.forward = 0.0;
        input.0.strafe = 0.0;
        input.0.sprint = false;
        input.0.sneak = false;
        input.0.jump = false;
        timings.input_collect_ms = timer.ms();
        return;
    }

    let mut look_delta = Vec2::ZERO;
    for ev in motion_events.read() {
        look_delta += ev.delta;
    }

    let sensitivity = 0.002;
    // Bevy uses right-handed yaw (positive rotates left), so invert mouse X for FPS feel.
    input.0.yaw -= look_delta.x * sensitivity;
    // Mouse up should look up.
    input.0.pitch -= look_delta.y * sensitivity;
    input.0.pitch = input.0.pitch.clamp(-1.54, 1.54);

    input.0.forward = 0.0;
    input.0.strafe = 0.0;
    if keys.pressed(KeyCode::KeyW) {
        input.0.forward += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        input.0.forward -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        input.0.strafe += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        input.0.strafe -= 1.0;
    }

    if matches!(perspective.mode, CameraPerspectiveMode::ThirdPersonFront) {
        // Front-facing third-person camera is rotated 180deg around the player.
        // Map movement to camera-relative controls for this view.
        input.0.forward = -input.0.forward;
        input.0.strafe = -input.0.strafe;
    }

    input.0.jump = keys.pressed(KeyCode::Space);

    input.0.sprint = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    input.0.sneak = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    timings.input_collect_ms = timer.ms();
}

pub fn camera_zoom_system(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut wheel: EventReader<MouseWheel>,
    app_state: Res<AppState>,
    ui_state: Res<UiState>,
    player_status: Res<rs_utils::PlayerStatus>,
    render_debug: Res<RenderDebugSettings>,
    mut zoom: ResMut<ZoomState>,
    mut cameras: Query<&mut Projection, With<PlayerCamera>>,
) {
    // Drain wheel events every frame to avoid scroll backlog when zoom isn't active.
    let mut wheel_delta = 0.0f32;
    for ev in wheel.read() {
        wheel_delta += ev.y;
    }

    let input_allowed = matches!(app_state.0, ApplicationState::Connected)
        && !ui_state.chat_open
        && !ui_state.paused
        && !ui_state.inventory_open
        && !player_status.dead;

    const BASE_ZOOM_FACTOR: f32 = 2.0; // 200% zoom
    const MIN_ZOOM_FACTOR: f32 = 1.1;
    const MAX_ZOOM_FACTOR: f32 = 12.0;

    if !input_allowed {
        zoom.active = false;
        zoom.target_factor = 1.0;
        zoom.wheel_factor = 1.0;
    } else {
        if keys.just_pressed(KeyCode::KeyC) {
            zoom.active = true;
            zoom.wheel_factor = 1.0;
            zoom.target_factor = BASE_ZOOM_FACTOR;
        }

        if keys.just_released(KeyCode::KeyC) {
            zoom.active = false;
            zoom.target_factor = 1.0;
        }

        if keys.pressed(KeyCode::KeyC) {
            zoom.active = true;
            if wheel_delta.abs() > f32::EPSILON {
                // Wheel up zooms in further (higher factor), wheel down zooms out (lower factor).
                zoom.wheel_factor *= 1.1f32.powf(wheel_delta);
                zoom.wheel_factor = zoom
                    .wheel_factor
                    .clamp(0.6, MAX_ZOOM_FACTOR / BASE_ZOOM_FACTOR);
            }
            zoom.target_factor =
                (BASE_ZOOM_FACTOR * zoom.wheel_factor).clamp(MIN_ZOOM_FACTOR, MAX_ZOOM_FACTOR);
        }
    }

    // Smooth factor transition (exponential smoothing).
    let dt = time.delta_secs().clamp(0.0, 0.05);
    let alpha = 1.0 - (-22.0 * dt).exp();
    zoom.current_factor += (zoom.target_factor - zoom.current_factor) * alpha;
    if (zoom.current_factor - zoom.target_factor).abs() < 0.0005 {
        zoom.current_factor = zoom.target_factor;
    }
    zoom.current_factor = zoom.current_factor.clamp(1.0, MAX_ZOOM_FACTOR);

    let base_fov = render_debug.fov_deg.max(1.0).to_radians();
    let desired_fov = (base_fov / zoom.current_factor).clamp(0.01, std::f32::consts::PI - 0.01);
    for mut projection in &mut cameras {
        if let Projection::Perspective(p) = &mut *projection {
            p.fov = desired_fov;
        }
    }
}

#[derive(Component)]
pub struct LocalHeldItemSprite;

pub fn local_held_item_view_system(
    mut commands: Commands,
    app_state: Res<AppState>,
    ui_state: Res<UiState>,
    player_status: Res<rs_utils::PlayerStatus>,
    render_debug: Res<RenderDebugSettings>,
    perspective: Res<CameraPerspectiveState>,
    inventory: Res<InventoryState>,
    mut item_textures: ResMut<ItemTextureCache>,
    item_sprite_mesh: Res<ItemSpriteMesh>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    camera: Query<Entity, With<PlayerCamera>>,
    existing: Query<Entity, With<LocalHeldItemSprite>>,
) {
    if !matches!(app_state.0, ApplicationState::Connected)
        || ui_state.chat_open
        || ui_state.paused
        || ui_state.inventory_open
        || player_status.dead
        || !render_debug.render_held_items
        || render_debug.render_first_person_arms
        || !matches!(perspective.mode, CameraPerspectiveMode::FirstPerson)
    {
        for e in existing.iter() {
            commands.entity(e).despawn_recursive();
        }
        return;
    }

    let Ok(cam_entity) = camera.get_single() else {
        return;
    };

    let held = inventory.hotbar_item(inventory.selected_hotbar_slot);
    let Some(stack) = held else {
        for e in existing.iter() {
            commands.entity(e).despawn_recursive();
        }
        return;
    };

    item_textures.request_stack(&stack);
    let sprite_entity = existing.iter().next();
    if let Some(e) = sprite_entity {
        commands.entity(e).insert(ItemSpriteStack(stack));
        return;
    }

    let placeholder = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        alpha_mode: AlphaMode::Mask(0.5),
        cull_mode: None,
        unlit: true,
        perceptual_roughness: 1.0,
        metallic: 0.0,
        ..Default::default()
    });
    let e = commands
        .spawn((
            Name::new("LocalHeldItem"),
            Mesh3d(item_sprite_mesh.0.clone()),
            MeshMaterial3d(placeholder),
            Transform {
                translation: Vec3::new(0.42, -0.36, -0.70),
                rotation: Quat::from_rotation_x(-0.55) * Quat::from_rotation_y(0.40),
                scale: Vec3::splat(0.70),
            },
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::default(),
            ViewVisibility::default(),
            LocalHeldItemSprite,
            ItemSpriteStack(stack),
        ))
        .id();
    commands.entity(cam_entity).add_child(e);
}

pub fn frame_timing_start(
    time: Res<Time>,
    mut state: ResMut<FrameTimingState>,
    mut timings: ResMut<PerfTimings>,
) {
    state.start = Some(Instant::now());
    timings.frame_delta_ms = time.delta_secs() * 1000.0;
}

pub fn frame_timing_end(mut state: ResMut<FrameTimingState>, mut timings: ResMut<PerfTimings>) {
    if let Some(start) = state.start.take() {
        timings.main_thread_ms = start.elapsed().as_secs_f32() * 1000.0;
    }
}

pub fn update_timing_start(mut state: ResMut<FrameTimingState>) {
    state.update_start = Some(Instant::now());
}

pub fn update_timing_end(mut state: ResMut<FrameTimingState>, mut timings: ResMut<PerfTimings>) {
    if let Some(start) = state.update_start.take() {
        timings.update_ms = start.elapsed().as_secs_f32() * 1000.0;
    }
}

pub fn post_update_timing_start(mut state: ResMut<FrameTimingState>) {
    state.post_update_start = Some(Instant::now());
}

pub fn post_update_timing_end(
    mut state: ResMut<FrameTimingState>,
    mut timings: ResMut<PerfTimings>,
) {
    if let Some(start) = state.post_update_start.take() {
        timings.post_update_ms = start.elapsed().as_secs_f32() * 1000.0;
    }
}

pub fn fixed_update_timing_start(mut state: ResMut<FrameTimingState>) {
    state.fixed_update_start = Some(Instant::now());
}

pub fn fixed_update_timing_end(
    mut state: ResMut<FrameTimingState>,
    mut timings: ResMut<PerfTimings>,
) {
    if let Some(start) = state.fixed_update_start.take() {
        timings.fixed_update_ms = start.elapsed().as_secs_f32() * 1000.0;
    }
}

pub fn debug_toggle_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut debug_ui: ResMut<DebugUiState>,
    mut hitbox_debug: ResMut<EntityHitboxDebug>,
    ui_state: Res<UiState>,
) {
    if ui_state.chat_open || ui_state.inventory_open {
        return;
    }
    if keys.just_pressed(KeyCode::KeyF) {
        debug_ui.open = !debug_ui.open;
    }
    if keys.just_pressed(KeyCode::KeyH) {
        hitbox_debug.enabled = !hitbox_debug.enabled;
    }
}

pub fn camera_perspective_toggle_system(
    keys: Res<ButtonInput<KeyCode>>,
    ui_state: Res<UiState>,
    mut perspective: ResMut<CameraPerspectiveState>,
    mut alt_hold: ResMut<CameraPerspectiveAltHold>,
) {
    if ui_state.chat_open || ui_state.inventory_open || ui_state.paused {
        return;
    }

    let alt_pressed = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    if alt_pressed || !keys.just_pressed(KeyCode::F5) {
        return;
    }

    let cycle = |mode| match mode {
        CameraPerspectiveMode::FirstPerson => CameraPerspectiveMode::ThirdPersonBack,
        CameraPerspectiveMode::ThirdPersonBack => CameraPerspectiveMode::ThirdPersonFront,
        CameraPerspectiveMode::ThirdPersonFront => CameraPerspectiveMode::FirstPerson,
    };
    perspective.mode = cycle(perspective.mode);
    if let Some(saved) = alt_hold.saved_mode {
        alt_hold.saved_mode = Some(cycle(saved));
    }
}

pub fn camera_perspective_alt_hold_system(
    keys: Res<ButtonInput<KeyCode>>,
    ui_state: Res<UiState>,
    mut perspective: ResMut<CameraPerspectiveState>,
    mut alt_hold: ResMut<CameraPerspectiveAltHold>,
) {
    if ui_state.chat_open || ui_state.inventory_open || ui_state.paused {
        alt_hold.saved_mode = None;
        return;
    }

    let alt_pressed = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    if alt_pressed {
        if alt_hold.saved_mode.is_none() {
            alt_hold.saved_mode = Some(perspective.mode);
        }
        perspective.mode = CameraPerspectiveMode::ThirdPersonBack;
    } else if let Some(saved) = alt_hold.saved_mode.take() {
        perspective.mode = saved;
    }
}

pub fn fixed_sim_tick_system(
    mut sim_clock: ResMut<SimClock>,
    mut sim_render: ResMut<SimRenderState>,
    mut sim_state: ResMut<SimState>,
    mut input: ResMut<CurrentInput>,
    mut history: ResMut<PredictionHistory>,
    mut latency: ResMut<LatencyEstimate>,
    mut action_state: ResMut<ActionState>,
    mut player_status: ResMut<rs_utils::PlayerStatus>,
    mut correction_guard: ResMut<CorrectionLoopGuard>,
    mut move_pkt_state: ResMut<MovementPacketState>,
    collision_map: Res<WorldCollisionMap>,
    app_state: Res<AppState>,
    to_net: Res<ToNet>,
    remote_entities: Res<RemoteEntityRegistry>,
    sim_ready: Res<crate::sim::SimReady>,
    mut timings: ResMut<PerfTimings>,
) {
    let timer = Timing::start();
    if !matches!(app_state.0, ApplicationState::Connected) || !sim_ready.0 {
        move_pkt_state.initialized = false;
        move_pkt_state.ticks_since_pos = 0;
        action_state.jump_was_pressed = false;
        action_state.fly_toggle_timer = 0;
        timings.fixed_tick_ms = timer.ms();
        return;
    }
    let world = WorldCollision::with_map(&collision_map);
    let tick = sim_clock.tick;
    let mut input_snapshot = input.0;

    let gamemode_allows_flight = matches!(player_status.gamemode, 1 | 3);
    if !gamemode_allows_flight {
        player_status.can_fly = false;
        player_status.flying = false;
        action_state.fly_toggle_timer = 0;
    }

    let jump_pressed = input_snapshot.jump && !action_state.jump_was_pressed;
    action_state.jump_was_pressed = input_snapshot.jump;

    if action_state.fly_toggle_timer > 0 {
        action_state.fly_toggle_timer = action_state.fly_toggle_timer.saturating_sub(1);
    }
    if player_status.can_fly {
        if jump_pressed {
            if action_state.fly_toggle_timer == 0 {
                // Vanilla 1.8: second jump press within 7 ticks toggles creative flight.
                action_state.fly_toggle_timer = 7;
            } else {
                player_status.flying = !player_status.flying;
                action_state.fly_toggle_timer = 0;
                let _ = to_net.0.send(ToNetMessage::ClientAbilities {
                    flags: client_abilities_flags(&player_status),
                    flying_speed: player_status.flying_speed,
                    walking_speed: player_status.walking_speed,
                });
            }
        }
    } else if player_status.flying {
        player_status.flying = false;
        action_state.fly_toggle_timer = 0;
        let _ = to_net.0.send(ToNetMessage::ClientAbilities {
            flags: client_abilities_flags(&player_status),
            flying_speed: player_status.flying_speed,
            walking_speed: player_status.walking_speed,
        });
    }

    input_snapshot.can_fly = player_status.can_fly;
    input_snapshot.flying = player_status.flying;
    input_snapshot.flying_speed = player_status.flying_speed;
    input_snapshot.speed_multiplier = match player_status.speed_effect_amplifier {
        Some(amplifier) => 1.0 + 0.2 * (f32::from(amplifier) + 1.0),
        None => 1.0,
    };
    input_snapshot.jump_boost_amplifier = player_status.jump_boost_amplifier;

    // Vanilla-like sprint state latching:
    // - start requires strong forward input and movement
    // - while already sprinting, keep sprint as long as sprint key is held and moving forward
    // This avoids rapid sprint state flapping around sprint-jumps.
    let horizontal_speed_sq = sim_state.current.vel.x * sim_state.current.vel.x
        + sim_state.current.vel.z * sim_state.current.vel.z;
    let can_start_sprint = input_snapshot.sprint
        && !input_snapshot.sneak
        && input_snapshot.forward >= 0.8
        && horizontal_speed_sq > 1.0e-5;
    let can_keep_sprint = action_state.sprinting
        && input_snapshot.sprint
        && !input_snapshot.sneak
        && input_snapshot.forward > 0.0;
    let sprinting_state = can_start_sprint || can_keep_sprint;
    input_snapshot.sprint = sprinting_state;

    sim_render.previous = sim_state.current;
    let mut next_state = if correction_guard.skip_physics_ticks > 0 {
        correction_guard.skip_physics_ticks = correction_guard.skip_physics_ticks.saturating_sub(1);
        let mut state = sim_state.current;
        // During correction settle, pin once to the last authoritative server pose.
        state.pos = correction_guard.last_server_pos;
        state.on_ground = correction_guard.last_server_on_ground;
        state.yaw = input_snapshot.yaw;
        state.pitch = input_snapshot.pitch;
        state.vel = Vec3::ZERO;
        state
    } else {
        simulate_tick(&sim_state.current, &input_snapshot, &world)
    };

    history.0.push(PredictedFrame {
        tick,
        input: input_snapshot,
        state: next_state,
    });

    sim_state.current = next_state;
    if player_status.flying && player_status.gamemode != 3 && sim_state.current.on_ground {
        // Vanilla behavior: landing cancels flying outside spectator mode.
        player_status.flying = false;
        let _ = to_net.0.send(ToNetMessage::ClientAbilities {
            flags: client_abilities_flags(&player_status),
            flying_speed: player_status.flying_speed,
            walking_speed: player_status.walking_speed,
        });
    }
    sim_clock.tick = sim_clock.tick.wrapping_add(1);
    input.0.jump = false;

    if matches!(app_state.0, ApplicationState::Connected) {
        if let Some((ack_pos, ack_yaw, ack_pitch, ack_on_ground)) =
            correction_guard.pending_ack.take()
        {
            let _ = to_net.0.send(ToNetMessage::PlayerMovePosLook {
                x: ack_pos.x as f64,
                y: ack_pos.y as f64,
                z: ack_pos.z as f64,
                yaw: ack_yaw,
                pitch: ack_pitch,
                on_ground: ack_on_ground,
            });
            latency.last_sent = Some(Instant::now());
            timings.fixed_tick_ms = timer.ms();
            return;
        }
        if correction_guard.skip_send_ticks > 0 {
            correction_guard.skip_send_ticks = correction_guard.skip_send_ticks.saturating_sub(1);
            timings.fixed_tick_ms = timer.ms();
            return;
        }
        let current_sneak = input_snapshot.sneak;
        let current_sprint = effective_sprint(&input_snapshot);

        if current_sneak != action_state.sneaking {
            let action_id = if current_sneak { 0 } else { 1 };
            if let Some(entity_id) = remote_entities.local_entity_id {
                let _ = to_net.0.send(ToNetMessage::PlayerAction {
                    entity_id,
                    action_id,
                });
            }
            action_state.sneaking = current_sneak;
        }
        if current_sprint != action_state.sprinting {
            let action_id = if current_sprint { 3 } else { 4 };
            if let Some(entity_id) = remote_entities.local_entity_id {
                let _ = to_net.0.send(ToNetMessage::PlayerAction {
                    entity_id,
                    action_id,
                });
            }
            action_state.sprinting = current_sprint;
        }

        let pos = sim_state.current.pos;
        let mut yaw = wrap_degrees((std::f32::consts::PI - sim_state.current.yaw).to_degrees());
        let mut pitch = -sim_state.current.pitch.to_degrees();
        if !yaw.is_finite() {
            yaw = 0.0;
        }
        if !pitch.is_finite() {
            pitch = 0.0;
        }
        pitch = pitch.clamp(-90.0, 90.0);
        let on_ground = sim_state.current.on_ground;

        const POS_DELTA_SQ_EPS: f32 = 0.0009; // Vanilla-style 9e-4 threshold.

        correction_guard.force_full_pos_ticks = 0;

        let moved = if move_pkt_state.initialized {
            pos.distance_squared(move_pkt_state.last_pos) > POS_DELTA_SQ_EPS
                || move_pkt_state.ticks_since_pos >= 20
        } else {
            true
        };
        let rotated = if move_pkt_state.initialized {
            (yaw - move_pkt_state.last_yaw_deg).abs() > 0.001
                || (pitch - move_pkt_state.last_pitch_deg).abs() > 0.001
        } else {
            true
        };

        if moved && rotated {
            let _ = to_net.0.send(ToNetMessage::PlayerMovePosLook {
                x: pos.x as f64,
                y: pos.y as f64,
                z: pos.z as f64,
                yaw,
                pitch,
                on_ground,
            });
        } else if moved {
            let _ = to_net.0.send(ToNetMessage::PlayerMovePos {
                x: pos.x as f64,
                y: pos.y as f64,
                z: pos.z as f64,
                on_ground,
            });
        } else if rotated {
            let _ = to_net.0.send(ToNetMessage::PlayerMoveLook {
                yaw,
                pitch,
                on_ground,
            });
        } else {
            let _ = to_net.0.send(ToNetMessage::PlayerMoveGround { on_ground });
        }

        if moved {
            move_pkt_state.last_pos = pos;
            move_pkt_state.ticks_since_pos = 0;
        } else {
            move_pkt_state.ticks_since_pos = move_pkt_state.ticks_since_pos.saturating_add(1);
        }
        move_pkt_state.last_yaw_deg = yaw;
        move_pkt_state.last_pitch_deg = pitch;
        move_pkt_state.initialized = true;
        latency.last_sent = Some(Instant::now());
    }
    timings.fixed_tick_ms = timer.ms();
}

pub fn local_arm_swing_tick_system(time: Res<Time>, mut swing: ResMut<LocalArmSwing>) {
    if swing.progress >= 1.0 {
        swing.progress = 1.0;
        return;
    }
    let dt = time.delta_secs().clamp(0.0, 0.05);
    swing.progress = (swing.progress + dt * 3.6).min(1.0);
}

pub fn net_event_apply_system(
    mut net_events: ResMut<NetEventQueue>,
    mut sim_render: ResMut<SimRenderState>,
    mut sim_state: ResMut<SimState>,
    mut history: ResMut<PredictionHistory>,
    mut visual_offset: ResMut<VisualCorrectionOffset>,
    mut debug: ResMut<DebugStats>,
    mut latency: ResMut<LatencyEstimate>,
    mut correction_guard: ResMut<CorrectionLoopGuard>,
    mut move_pkt_state: ResMut<MovementPacketState>,
    mut sim_ready: ResMut<crate::sim::SimReady>,
    mut timings: ResMut<PerfTimings>,
) {
    let timer = Timing::start();
    for event in net_events.drain() {
        let (pos, yaw, pitch, on_ground, recv_instant) = match event {
            crate::net::events::NetEvent::ServerPosLook {
                pos,
                yaw,
                pitch,
                on_ground,
                recv_instant,
            } => (pos, yaw, pitch, on_ground, recv_instant),
            crate::net::events::NetEvent::ServerVelocity { velocity } => {
                sim_state.current.vel = velocity;
                continue;
            }
        };
        if let Some(last_sent) = latency.last_sent {
            let rtt = recv_instant.saturating_duration_since(last_sent);
            let one_way = rtt.as_secs_f32() * 0.5;
            latency.one_way_ticks = (one_way / 0.05).round() as u32;
            debug.one_way_ticks = latency.one_way_ticks;
        }

        let server_state = crate::sim::PlayerSimState {
            pos,
            vel: Vec3::ZERO,
            on_ground,
            yaw,
            pitch,
        };

        let repeated_same_correction = pos.distance_squared(correction_guard.last_server_pos)
            <= 1.0e-6
            && on_ground == correction_guard.last_server_on_ground;
        correction_guard.repeats = if repeated_same_correction {
            correction_guard.repeats.saturating_add(1)
        } else {
            0
        };
        correction_guard.last_server_pos = pos;
        correction_guard.last_server_on_ground = on_ground;
        correction_guard.force_full_pos_ticks = 0;
        // Do not pause physics here: repeated correction bursts can otherwise
        // freeze the player in-air. We rely on authoritative snap + ACK.
        correction_guard.skip_physics_ticks = 0;

        // Queue one correction ACK for the next fixed send tick (avoid duplicate C03s/frame).
        let resolved_yaw_deg = wrap_degrees((std::f32::consts::PI - yaw).to_degrees());
        let resolved_pitch_deg = (-pitch.to_degrees()).clamp(-90.0, 90.0);
        let ack_yaw_deg = resolved_yaw_deg;
        let ack_pitch_deg = resolved_pitch_deg;
        correction_guard.pending_ack = Some((pos, ack_yaw_deg, ack_pitch_deg, on_ground));

        // Server correction packets are authoritative; snap to them directly.
        sim_render.previous = server_state;
        sim_state.current = server_state;
        history.0 = PredictionHistory::default().0;
        visual_offset.0 = Vec3::ZERO;
        sim_ready.0 = true;
        move_pkt_state.initialized = true;
        move_pkt_state.last_pos = pos;
        move_pkt_state.last_yaw_deg = ack_yaw_deg;
        move_pkt_state.last_pitch_deg = ack_pitch_deg;
        move_pkt_state.ticks_since_pos = 0;
        correction_guard.skip_send_ticks = 0;

        debug.last_correction = 0.0;
        debug.last_replay = 0;
    }
    timings.net_apply_ms = timer.ms();
}

pub fn visual_smoothing_system(
    time: Res<Time>,
    mut offset: ResMut<VisualCorrectionOffset>,
    mut debug: ResMut<DebugStats>,
    mut timings: ResMut<PerfTimings>,
) {
    let timer = Timing::start();
    let decay = 0.15f32;
    let factor = (1.0 - decay).powf(time.delta_secs() * 20.0);
    offset.0 *= factor;
    debug.smoothing_offset_len = offset.0.length();
    timings.smoothing_ms = timer.ms();
}

pub fn apply_visual_transform_system(
    fixed_time: Res<Time<Fixed>>,
    input: Res<CurrentInput>,
    perspective: Res<CameraPerspectiveState>,
    sim_render: Res<SimRenderState>,
    sim_state: Res<SimState>,
    offset: Res<VisualCorrectionOffset>,
    collision_map: Res<WorldCollisionMap>,
    mut player_query: Query<(&mut Transform, &mut LookAngles), With<Player>>,
    mut camera_query: Query<&mut Transform, (With<PlayerCamera>, Without<Player>)>,
    mut eye_height: Local<f32>,
    mut timings: ResMut<PerfTimings>,
) {
    let timer = Timing::start();
    const EYE_HEIGHT_STAND: f32 = 1.62;
    const EYE_HEIGHT_SNEAK: f32 = 1.54;
    let alpha = fixed_time.overstep_fraction().clamp(0.0, 1.0);
    if let Ok((mut player_transform, mut look)) = player_query.get_single_mut() {
        let interpolated = sim_render.previous.pos.lerp(sim_state.current.pos, alpha);
        let pos = interpolated + offset.0;
        player_transform.translation = pos;
        look.yaw = input.0.yaw;
        look.pitch = input.0.pitch;
        player_transform.rotation = Quat::from_axis_angle(Vec3::Y, look.yaw);
        if let Ok(mut camera_transform) = camera_query.get_single_mut() {
            let target_eye_height = if input.0.sneak {
                EYE_HEIGHT_SNEAK
            } else {
                EYE_HEIGHT_STAND
            };
            if *eye_height <= 0.0 {
                *eye_height = target_eye_height;
            } else {
                *eye_height += (target_eye_height - *eye_height) * 0.5;
            }

            let pitch_rot = Quat::from_axis_angle(Vec3::X, look.pitch);
            let base_eye_local = Vec3::new(0.0, *eye_height, 0.0);

            let (mut cam_local, cam_local_rot) = match perspective.mode {
                CameraPerspectiveMode::FirstPerson => (base_eye_local, pitch_rot),
                CameraPerspectiveMode::ThirdPersonBack => {
                    let offset_local =
                        pitch_rot * Vec3::new(0.0, 0.0, perspective.third_person_distance);
                    (base_eye_local + offset_local, pitch_rot)
                }
                CameraPerspectiveMode::ThirdPersonFront => {
                    let offset_local =
                        pitch_rot * Vec3::new(0.0, 0.0, -perspective.third_person_distance);
                    (
                        base_eye_local + offset_local,
                        pitch_rot * Quat::from_rotation_y(std::f32::consts::PI),
                    )
                }
            };

            // Third-person camera collision: shorten the camera distance when a solid block is in the way.
            if !matches!(perspective.mode, CameraPerspectiveMode::FirstPerson) {
                let player_rot = player_transform.rotation;
                let anchor_world = player_transform.translation + Vec3::Y * *eye_height;
                let desired_world = player_transform.translation + (player_rot * cam_local);
                let clipped_world =
                    clip_camera_to_world(&collision_map, anchor_world, desired_world);
                cam_local = player_rot.inverse() * (clipped_world - player_transform.translation);
            }

            camera_transform.translation = cam_local;
            camera_transform.rotation = cam_local_rot;
        }
    }
    timings.apply_transform_ms = timer.ms();
}

fn clip_camera_to_world(world: &WorldCollisionMap, anchor: Vec3, desired: Vec3) -> Vec3 {
    let delta = desired - anchor;
    let dist = delta.length();
    if dist <= 0.001 {
        return desired;
    }

    let dir = delta / dist;
    let step = 0.08f32;
    let mut t = step;
    let mut prev_cell = anchor.floor().as_ivec3();
    while t <= dist {
        let point = anchor + dir * t;
        let cell = point.floor().as_ivec3();
        if cell != prev_cell {
            let block_state = world.block_at(cell.x, cell.y, cell.z);
            if crate::sim::collision::is_solid(block_state) {
                // Keep a small offset so the camera doesn't sit exactly on the block face.
                return anchor + dir * (t - 0.12).max(0.0);
            }
            prev_cell = cell;
        }
        t += step;
    }

    desired
}

pub fn world_interaction_system(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    app_state: Res<AppState>,
    ui_state: Res<UiState>,
    player_status: Res<rs_utils::PlayerStatus>,
    to_net: Res<ToNet>,
    mut inventory_state: ResMut<InventoryState>,
    sim_state: Res<SimState>,
    mut swing: ResMut<LocalArmSwing>,
    mut break_indicator: ResMut<BreakIndicator>,
    collision_map: Res<WorldCollisionMap>,
    camera_query: Query<&GlobalTransform, With<PlayerCamera>>,
    remote_entities: Query<(&GlobalTransform, &RemoteEntity, &RemoteVisual)>,
    mut mining: Local<MiningState>,
) {
    if !matches!(app_state.0, ApplicationState::Connected)
        || ui_state.chat_open
        || ui_state.paused
        || ui_state.inventory_open
        || player_status.dead
    {
        if mining.active {
            let _ = to_net.0.send(ToNetMessage::DigCancel {
                x: mining.target_block.x,
                y: mining.target_block.y,
                z: mining.target_block.z,
                face: mining.face,
            });
            *mining = MiningState::default();
        }
        *break_indicator = BreakIndicator::default();
        return;
    }

    let left_just_pressed = mouse.just_pressed(MouseButton::Left);
    let left_held = mouse.pressed(MouseButton::Left);
    let right_click = mouse.just_pressed(MouseButton::Right);
    if !left_held && !right_click {
        if mining.active {
            let _ = to_net.0.send(ToNetMessage::DigCancel {
                x: mining.target_block.x,
                y: mining.target_block.y,
                z: mining.target_block.z,
                face: mining.face,
            });
            *mining = MiningState::default();
        }
        *break_indicator = BreakIndicator::default();
        return;
    }

    let Ok(camera_transform) = camera_query.get_single() else {
        *break_indicator = BreakIndicator::default();
        return;
    };
    let origin = camera_transform.translation();
    let dir = *camera_transform.forward();
    let is_creative = player_status.gamemode == 1;
    let block_reach = if is_creative {
        CREATIVE_BLOCK_REACH
    } else {
        SURVIVAL_BLOCK_REACH
    };
    let entity_reach = if is_creative {
        CREATIVE_ENTITY_REACH
    } else {
        SURVIVAL_ENTITY_REACH
    };

    let block_hit = raycast_block(&collision_map, origin, dir, block_reach);
    let entity_hit = raycast_remote_entity(&remote_entities, origin, dir, entity_reach);

    if left_just_pressed {
        let _ = to_net.0.send(ToNetMessage::SwingArm);
        swing.progress = 0.0;
    }

    if left_held {
        if let Some(entity) = nearest_entity_hit(entity_hit, block_hit) {
            if left_just_pressed {
                let _ = to_net.0.send(ToNetMessage::UseEntity {
                    target_id: entity.entity_id,
                    action: EntityUseAction::Attack,
                });
            }
            if mining.active {
                let _ = to_net.0.send(ToNetMessage::DigCancel {
                    x: mining.target_block.x,
                    y: mining.target_block.y,
                    z: mining.target_block.z,
                    face: mining.face,
                });
                *mining = MiningState::default();
            }
            *break_indicator = BreakIndicator::default();
        } else if let Some(hit) = block_hit {
            let face = normal_to_face_index(hit.normal);
            let block_id = collision_map.block_at(hit.block.x, hit.block.y, hit.block.z);
            let held_item = inventory_state.hotbar_item(inventory_state.selected_hotbar_slot);
            let total_secs = estimate_break_time_secs(block_id, held_item);

            if !mining.active || mining.target_block != hit.block || mining.face != face {
                if mining.active {
                    let _ = to_net.0.send(ToNetMessage::DigCancel {
                        x: mining.target_block.x,
                        y: mining.target_block.y,
                        z: mining.target_block.z,
                        face: mining.face,
                    });
                }
                *mining = MiningState {
                    active: true,
                    target_block: hit.block,
                    face,
                    elapsed_secs: 0.0,
                    total_secs,
                    finish_sent: false,
                };
                let _ = to_net.0.send(ToNetMessage::DigStart {
                    x: hit.block.x,
                    y: hit.block.y,
                    z: hit.block.z,
                    face,
                });
            } else {
                mining.elapsed_secs += time.delta_secs();
            }

            let progress = if mining.total_secs > 0.0 {
                (mining.elapsed_secs / mining.total_secs).clamp(0.0, 1.0)
            } else {
                1.0
            };
            *break_indicator = BreakIndicator {
                active: true,
                progress,
                elapsed_secs: mining.elapsed_secs,
                total_secs: mining.total_secs,
            };

            if !mining.finish_sent && progress >= 1.0 {
                let _ = to_net.0.send(ToNetMessage::DigFinish {
                    x: mining.target_block.x,
                    y: mining.target_block.y,
                    z: mining.target_block.z,
                    face: mining.face,
                });
                mining.finish_sent = true;
            }
        } else {
            if mining.active {
                let _ = to_net.0.send(ToNetMessage::DigCancel {
                    x: mining.target_block.x,
                    y: mining.target_block.y,
                    z: mining.target_block.z,
                    face: mining.face,
                });
            }
            *mining = MiningState::default();
            *break_indicator = BreakIndicator::default();
        }
    } else if mining.active {
        let _ = to_net.0.send(ToNetMessage::DigCancel {
            x: mining.target_block.x,
            y: mining.target_block.y,
            z: mining.target_block.z,
            face: mining.face,
        });
        *mining = MiningState::default();
        *break_indicator = BreakIndicator::default();
    }

    if right_click {
        if let Some(entity) = nearest_entity_hit(entity_hit, block_hit) {
            let _ = to_net.0.send(ToNetMessage::UseEntity {
                target_id: entity.entity_id,
                action: EntityUseAction::Interact,
            });
        } else if let Some(hit) = block_hit {
            let face = normal_to_face_index(hit.normal);
            let target_state = collision_map.block_at(hit.block.x, hit.block.y, hit.block.z);
            let target_id = block_state_id(target_state);

            if is_interactable_block(target_id) {
                let _ = to_net.0.send(ToNetMessage::PlaceBlock {
                    x: hit.block.x,
                    y: hit.block.y,
                    z: hit.block.z,
                    face: face as i8,
                    cursor_x: 8,
                    cursor_y: 8,
                    cursor_z: 8,
                });
                return;
            }

            let place_pos = hit.block + hit.normal;
            if placement_intersects_player(place_pos, sim_state.current.pos) {
                return;
            }
            let _ = to_net.0.send(ToNetMessage::PlaceBlock {
                x: hit.block.x,
                y: hit.block.y,
                z: hit.block.z,
                face: face as i8,
                cursor_x: 8,
                cursor_y: 8,
                cursor_z: 8,
            });
            if player_status.gamemode != 1 {
                let _ = inventory_state.consume_selected_hotbar_one();
            }
        } else {
            let held_item = inventory_state.hotbar_item(inventory_state.selected_hotbar_slot);
            let _ = to_net.0.send(ToNetMessage::UseItem { held_item });
        }
    }
}

fn is_interactable_block(block_id: u16) -> bool {
    matches!(
        block_id,
        // Containers
        23 | 54 | 61 | 62 | 84 | 130 | 146 | 154 | 158
            // Utility blocks
            | 58 | 116 | 117 | 118 | 120 | 145
            // Doors, trapdoors, fence gates
            | 64 | 71 | 96 | 107 | 167 | 183 | 184 | 185 | 186 | 187 | 193 | 194 | 195 | 196 | 197
            // Buttons/levers
            | 69 | 77 | 143
            // Note block
            | 25
    )
}

fn placement_intersects_player(block_pos: IVec3, player_feet: Vec3) -> bool {
    const PLAYER_HALF_WIDTH: f32 = 0.3;
    const PLAYER_HEIGHT: f32 = 1.8;
    let player_min = Vec3::new(
        player_feet.x - PLAYER_HALF_WIDTH,
        player_feet.y,
        player_feet.z - PLAYER_HALF_WIDTH,
    );
    let player_max = Vec3::new(
        player_feet.x + PLAYER_HALF_WIDTH,
        player_feet.y + PLAYER_HEIGHT,
        player_feet.z + PLAYER_HALF_WIDTH,
    );

    let block_min = Vec3::new(block_pos.x as f32, block_pos.y as f32, block_pos.z as f32);
    let block_max = block_min + Vec3::ONE;

    player_min.x < block_max.x
        && player_max.x > block_min.x
        && player_min.y < block_max.y
        && player_max.y > block_min.y
        && player_min.z < block_max.z
        && player_max.z > block_min.z
}

#[derive(Clone, Copy)]
struct RayHit {
    block: IVec3,
    normal: IVec3,
    distance: f32,
}

#[derive(Clone, Copy)]
struct EntityHit {
    entity_id: i32,
    distance: f32,
}

fn estimate_break_time_secs(block_id: u16, held_item: Option<rs_utils::InventoryItemStack>) -> f32 {
    let hardness = block_hardness(block_id);
    if hardness < 0.0 {
        return 9999.0;
    }
    if hardness == 0.0 {
        return 0.05;
    }
    let item_id = held_item.map(|stack| stack.item_id);
    let can_harvest = can_harvest_block(item_id, block_id);
    let speed = destroy_speed(item_id, block_id).max(0.1);
    let damage_per_tick = speed / hardness / if can_harvest { 30.0 } else { 100.0 };
    let ticks = (1.0 / damage_per_tick).ceil().max(1.0);
    (ticks / 20.0).clamp(0.05, 9999.0)
}

fn block_hardness(block_id: u16) -> f32 {
    match block_id {
        0 => 0.0,                                      // air
        1 | 4 => 2.0,                                  // stone, cobble
        2 => 0.6,                                      // grass
        3 => 0.5,                                      // dirt
        5 | 17 | 162 => 2.0,                           // planks/log
        12 => 0.5,                                     // sand
        13 => 0.6,                                     // gravel
        14 | 15 | 16 | 21 | 56 | 73 | 74 | 129 => 3.0, // ores
        18 | 161 => 0.2,                               // leaves
        20 => 0.3,                                     // glass
        24 | 45 => 0.8,                                // sandstone, brick
        49 => 50.0,                                    // obsidian
        50 => 0.0,                                     // torch
        54 => 2.5,                                     // chest
        58 => 2.5,                                     // crafting
        61 | 62 => 3.5,                                // furnace
        79 => 0.5,                                     // ice
        80 => 0.2,                                     // snow block
        81 => 0.4,                                     // cactus
        82 => 0.6,                                     // clay
        87 => 0.4,                                     // netherrack
        88 => 0.5,                                     // soulsand
        89 => 0.3,                                     // glowstone
        95 => 0.3,                                     // stained glass
        98 => 1.5,                                     // stone bricks
        155 => 0.8,                                    // quartz block
        159 => 0.8,                                    // stained hardened clay
        171 => 0.8,                                    // carpet/wool-ish break feel
        172 => 1.25,                                   // hardened clay
        173 => 5.0,                                    // coal block
        174 => 0.5,                                    // packed ice
        _ => 1.0,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ToolKind {
    Pickaxe,
    Shovel,
    Axe,
    Sword,
    Shears,
}

fn tool_kind(item_id: i32) -> Option<ToolKind> {
    match item_id {
        256 | 269 | 273 | 277 | 284 => Some(ToolKind::Shovel),
        257 | 270 | 274 | 278 | 285 => Some(ToolKind::Pickaxe),
        258 | 271 | 275 | 279 | 286 => Some(ToolKind::Axe),
        267 | 268 | 272 | 276 | 283 => Some(ToolKind::Sword),
        359 => Some(ToolKind::Shears),
        _ => None,
    }
}

fn tool_harvest_level(item_id: i32) -> i32 {
    match item_id {
        269 | 270 | 271 | 268 => 0, // wood / gold handled by speed
        273 | 274 | 275 | 272 => 1, // stone
        256..=258 | 267 => 2,       // iron
        277..=279 | 276 => 3,       // diamond
        284..=286 | 283 => 0,       // gold
        _ => -1,
    }
}

fn tool_speed(item_id: i32) -> f32 {
    match item_id {
        269..=271 | 268 => 2.0,
        273..=275 | 272 => 4.0,
        256..=258 | 267 => 6.0,
        277..=279 | 276 => 8.0,
        284..=286 | 283 => 12.0,
        359 => 5.0, // shears
        _ => 1.0,
    }
}

fn block_required_tool(block_id: u16) -> Option<(ToolKind, i32)> {
    match block_id {
        // Pickaxe, mostly stone/ore-like blocks.
        1 | 4 | 14 | 15 | 16 | 21 | 22 | 24 | 41 | 42 | 45 | 48 | 57 | 61 | 62 | 73 | 74 | 79
        | 80 | 98 | 101 | 109 | 112 | 121 | 133 | 152 | 155 | 172 | 173 => {
            Some((ToolKind::Pickaxe, 0))
        }
        // Higher harvest tiers.
        56 | 129 | 130 => Some((ToolKind::Pickaxe, 2)), // diamond/emerald/ender chest
        49 | 116 => Some((ToolKind::Pickaxe, 3)),       // obsidian/enchanting table
        // Shovel blocks.
        2 | 3 | 12 | 13 | 78 | 82 | 88 | 110 => Some((ToolKind::Shovel, 0)),
        // Axe blocks.
        5 | 17 | 47 | 53 | 54 | 58 | 64 | 96 | 107 | 134..=136 | 156 | 162 => {
            Some((ToolKind::Axe, 0))
        }
        _ => None,
    }
}

fn block_tool_bonus_kind(block_id: u16) -> Option<ToolKind> {
    match block_id {
        18 | 31 | 32 | 37 | 38 | 39 | 40 | 59 | 81 | 83 | 106 | 111 | 141 | 142 | 161 | 175 => {
            Some(ToolKind::Shears)
        }
        30 => Some(ToolKind::Sword), // cobweb
        _ => block_required_tool(block_id).map(|(kind, _)| kind),
    }
}

fn can_harvest_block(item_id: Option<i32>, block_id: u16) -> bool {
    let Some((required_kind, required_level)) = block_required_tool(block_id) else {
        return true;
    };
    let Some(item_id) = item_id else {
        return false;
    };
    if tool_kind(item_id) != Some(required_kind) {
        return false;
    }
    tool_harvest_level(item_id) >= required_level
}

fn destroy_speed(item_id: Option<i32>, block_id: u16) -> f32 {
    let Some(item_id) = item_id else {
        return 1.0;
    };
    let Some(kind) = tool_kind(item_id) else {
        return 1.0;
    };
    if block_tool_bonus_kind(block_id) == Some(kind) {
        tool_speed(item_id)
    } else {
        1.0
    }
}

fn nearest_entity_hit(
    entity_hit: Option<EntityHit>,
    block_hit: Option<RayHit>,
) -> Option<EntityHit> {
    match (entity_hit, block_hit) {
        (Some(entity), Some(block)) if entity.distance <= block.distance + 0.12 => Some(entity),
        (Some(_), Some(_)) => None,
        (Some(entity), None) => Some(entity),
        _ => None,
    }
}

fn raycast_block(
    world: &WorldCollisionMap,
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
) -> Option<RayHit> {
    let dir = direction.normalize_or_zero();
    if dir.length_squared() == 0.0 {
        return None;
    }

    let mut prev_cell = origin.floor().as_ivec3();
    let step = 0.05f32;
    let mut t = step;
    while t <= max_distance {
        let point = origin + dir * t;
        let cell = point.floor().as_ivec3();
        if cell != prev_cell {
            let block_state = world.block_at(cell.x, cell.y, cell.z);
            let block_id = block_state_id(block_state);
            if block_id != 0 && !matches!(block_id, 8 | 9 | 10 | 11) {
                let normal = prev_cell - cell;
                let normal = IVec3::new(normal.x.signum(), normal.y.signum(), normal.z.signum());
                return Some(RayHit {
                    block: cell,
                    normal,
                    distance: t,
                });
            }
            prev_cell = cell;
        }
        t += step;
    }
    None
}

fn raycast_remote_entity(
    remote_entities: &Query<(&GlobalTransform, &RemoteEntity, &RemoteVisual)>,
    origin: Vec3,
    direction: Vec3,
    max_distance: f32,
) -> Option<EntityHit> {
    let dir = direction.normalize_or_zero();
    if dir.length_squared() == 0.0 {
        return None;
    }

    let mut nearest: Option<EntityHit> = None;
    for (transform, remote, visual) in remote_entities.iter() {
        let (half_w, height) = match remote.kind {
            rs_utils::NetEntityKind::Player | rs_utils::NetEntityKind::Mob(_) => (0.34, 1.8),
            rs_utils::NetEntityKind::Item => (0.22, 0.35),
            rs_utils::NetEntityKind::ExperienceOrb => (0.18, 0.28),
            rs_utils::NetEntityKind::Object(_) => (0.28, 0.56),
        };
        // Remote entity transforms are rendered with a y-offset; derive collider base from that.
        let feet = transform.translation() - Vec3::Y * visual.y_offset;
        let min = Vec3::new(feet.x - half_w, feet.y, feet.z - half_w);
        let max = Vec3::new(feet.x + half_w, feet.y + height, feet.z + half_w);
        let Some(distance) = ray_aabb_distance(origin, dir, min, max, max_distance) else {
            continue;
        };

        match nearest {
            Some(current) if current.distance <= distance => {}
            _ => {
                nearest = Some(EntityHit {
                    entity_id: remote.server_id,
                    distance,
                });
            }
        }
    }

    nearest
}

pub fn draw_entity_hitboxes_system(
    mut gizmos: Gizmos,
    settings: Res<EntityHitboxDebug>,
    app_state: Res<AppState>,
    entities: Query<(&GlobalTransform, &RemoteEntity, &RemoteVisual)>,
) {
    if !settings.enabled || !matches!(app_state.0, ApplicationState::Connected) {
        return;
    }

    for (transform, remote, visual) in &entities {
        let (half_w, height) = match remote.kind {
            rs_utils::NetEntityKind::Player | rs_utils::NetEntityKind::Mob(_) => (0.34, 1.8),
            rs_utils::NetEntityKind::Item => (0.22, 0.35),
            rs_utils::NetEntityKind::ExperienceOrb => (0.18, 0.28),
            rs_utils::NetEntityKind::Object(_) => (0.28, 0.56),
        };
        let feet = transform.translation() - Vec3::Y * visual.y_offset;
        let min = Vec3::new(feet.x - half_w, feet.y, feet.z - half_w);
        let max = Vec3::new(feet.x + half_w, feet.y + height, feet.z + half_w);
        draw_aabb_lines(&mut gizmos, min, max, Color::srgba(0.2, 1.0, 0.2, 1.0));
    }
}

pub fn draw_chunk_debug_system(
    mut gizmos: Gizmos,
    render_debug: Res<RenderDebugSettings>,
    app_state: Res<AppState>,
    break_indicator: Res<BreakIndicator>,
    player_status: Res<rs_utils::PlayerStatus>,
    collision_map: Res<WorldCollisionMap>,
    chunks: Query<(&ChunkRoot, &Visibility)>,
    camera: Query<&GlobalTransform, With<PlayerCamera>>,
) {
    if !matches!(app_state.0, ApplicationState::Connected) {
        return;
    }

    if render_debug.show_chunk_borders {
        let color = Color::srgba(0.25, 0.75, 1.0, 1.0);
        for (chunk, vis) in &chunks {
            if matches!(*vis, Visibility::Hidden) {
                continue;
            }
            let base = Vec3::new(chunk.key.0 as f32 * 16.0, 0.0, chunk.key.1 as f32 * 16.0);
            let min = base;
            let max = base + Vec3::new(16.0, 256.0, 16.0);
            draw_aabb_lines(&mut gizmos, min, max, color);
        }
    }

    if render_debug.show_look_ray {
        let Ok(cam) = camera.get_single() else {
            return;
        };
        let origin = cam.translation();
        let dir = *cam.forward();
        let end = origin + dir * 3.0;
        gizmos.line(origin, end, Color::srgba(1.0, 0.2, 0.2, 1.0));
    }

    if render_debug.show_target_block_outline {
        let Ok(cam) = camera.get_single() else {
            return;
        };
        let origin = cam.translation();
        let dir = *cam.forward();
        let max_reach = if player_status.gamemode == 1 {
            CREATIVE_BLOCK_REACH
        } else {
            SURVIVAL_BLOCK_REACH
        };
        if let Some(hit) = raycast_block(&collision_map, origin, dir, max_reach) {
            let state = collision_map.block_at(hit.block.x, hit.block.y, hit.block.z);
            let world = WorldCollision::with_map(&collision_map);
            let boxes = target_outline_boxes(&world, state, hit.block.x, hit.block.y, hit.block.z);
            let inflate = 0.0025;
            for (mut min, mut max) in boxes.iter().copied() {
                min -= Vec3::splat(inflate);
                max += Vec3::splat(inflate);
                draw_aabb_lines(&mut gizmos, min, max, Color::srgba(0.02, 0.02, 0.02, 1.0));
            }
            if break_indicator.active {
                let p = break_indicator.progress.clamp(0.0, 1.0);
                let crack_color = Color::srgb(0.32 + 0.50 * p, 0.32 - 0.20 * p, 0.32 - 0.20 * p);
                let crack_inflate = 0.008 + p * 0.010;
                for (min, max) in boxes {
                    draw_aabb_lines(
                        &mut gizmos,
                        min - Vec3::splat(crack_inflate),
                        max + Vec3::splat(crack_inflate),
                        crack_color,
                    );
                }
            }
        }
    }
}

fn target_outline_boxes(
    world: &WorldCollision,
    block_state: u16,
    block_x: i32,
    block_y: i32,
    block_z: i32,
) -> Vec<(Vec3, Vec3)> {
    let mut boxes = debug_block_collision_boxes(world, block_state, block_x, block_y, block_z);
    if !boxes.is_empty() {
        return boxes;
    }

    let min = Vec3::new(block_x as f32, block_y as f32, block_z as f32);
    let max = min + Vec3::ONE;
    let id = block_state_id(block_state);

    let fallback = match block_model_kind(id) {
        rs_utils::BlockModelKind::Cross => {
            let h = if id == 175 { 1.0 } else { 0.875 };
            Some((min + Vec3::new(0.1, 0.0, 0.1), min + Vec3::new(0.9, h, 0.9)))
        }
        rs_utils::BlockModelKind::TorchLike => {
            Some((min + Vec3::new(0.4, 0.0, 0.4), min + Vec3::new(0.6, 0.75, 0.6)))
        }
        rs_utils::BlockModelKind::Custom => match id {
            26 => Some((min, min + Vec3::new(1.0, 9.0 / 16.0, 1.0))), // bed
            27 | 28 | 66 | 157 | 171 => Some((min, min + Vec3::new(1.0, 1.0 / 16.0, 1.0))), // rails/carpet
            78 => {
                let h = ((block_state_meta(block_state) & 0x7) as f32 + 1.0) / 8.0;
                Some((min, min + Vec3::new(1.0, h.clamp(0.125, 1.0), 1.0)))
            }
            _ => Some((min, max)),
        },
        _ => Some((min, max)),
    };

    if let Some(bb) = fallback {
        boxes.push(bb);
    }
    boxes
}

fn draw_aabb_lines(gizmos: &mut Gizmos, min: Vec3, max: Vec3, color: Color) {
    let p000 = Vec3::new(min.x, min.y, min.z);
    let p001 = Vec3::new(min.x, min.y, max.z);
    let p010 = Vec3::new(min.x, max.y, min.z);
    let p011 = Vec3::new(min.x, max.y, max.z);
    let p100 = Vec3::new(max.x, min.y, min.z);
    let p101 = Vec3::new(max.x, min.y, max.z);
    let p110 = Vec3::new(max.x, max.y, min.z);
    let p111 = Vec3::new(max.x, max.y, max.z);

    gizmos.line(p000, p001, color);
    gizmos.line(p001, p011, color);
    gizmos.line(p011, p010, color);
    gizmos.line(p010, p000, color);

    gizmos.line(p100, p101, color);
    gizmos.line(p101, p111, color);
    gizmos.line(p111, p110, color);
    gizmos.line(p110, p100, color);

    gizmos.line(p000, p100, color);
    gizmos.line(p001, p101, color);
    gizmos.line(p010, p110, color);
    gizmos.line(p011, p111, color);
}

fn ray_aabb_distance(
    origin: Vec3,
    dir: Vec3,
    min: Vec3,
    max: Vec3,
    max_distance: f32,
) -> Option<f32> {
    let mut t_min = 0.0f32;
    let mut t_max = max_distance;

    for axis in 0..3 {
        let (origin_axis, dir_axis, min_axis, max_axis) = match axis {
            0 => (origin.x, dir.x, min.x, max.x),
            1 => (origin.y, dir.y, min.y, max.y),
            _ => (origin.z, dir.z, min.z, max.z),
        };

        if dir_axis.abs() <= f32::EPSILON {
            if origin_axis < min_axis || origin_axis > max_axis {
                return None;
            }
            continue;
        }

        let inv = 1.0 / dir_axis;
        let mut t1 = (min_axis - origin_axis) * inv;
        let mut t2 = (max_axis - origin_axis) * inv;
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }

        t_min = t_min.max(t1);
        t_max = t_max.min(t2);
        if t_max < t_min {
            return None;
        }
    }

    if t_min <= max_distance {
        Some(t_min.max(0.0))
    } else {
        None
    }
}

fn normal_to_face_index(normal: IVec3) -> u8 {
    match (normal.x, normal.y, normal.z) {
        (0, -1, 0) => 0, // bottom
        (0, 1, 0) => 1,  // top
        (0, 0, -1) => 2, // north
        (0, 0, 1) => 3,  // south
        (-1, 0, 0) => 4, // west
        (1, 0, 0) => 5,  // east
        _ => 1,
    }
}

fn yaw_deg_to_cardinal(yaw_deg_mc: f32) -> (&'static str, &'static str) {
    // Minecraft 1.8 yaw: 0 = South (+Z), 90 = West (-X), 180 = North (-Z), 270 = East (+X).
    let yaw = yaw_deg_mc.rem_euclid(360.0);
    let idx = ((yaw / 90.0).round() as i32).rem_euclid(4);
    match idx {
        0 => ("South", "+Z"),
        1 => ("West", "-X"),
        2 => ("North", "-Z"),
        _ => ("East", "+X"),
    }
}

fn block_model_kind_label(kind: rs_utils::BlockModelKind) -> &'static str {
    match kind {
        rs_utils::BlockModelKind::FullCube => "FullCube",
        rs_utils::BlockModelKind::Cross => "Cross",
        rs_utils::BlockModelKind::Slab => "Slab",
        rs_utils::BlockModelKind::Stairs => "Stairs",
        rs_utils::BlockModelKind::Fence => "Fence",
        rs_utils::BlockModelKind::Pane => "Pane",
        rs_utils::BlockModelKind::Fluid => "Fluid",
        rs_utils::BlockModelKind::TorchLike => "TorchLike",
        rs_utils::BlockModelKind::Custom => "Custom",
    }
}

pub fn debug_overlay_system(
    mut contexts: EguiContexts,
    debug: Res<DebugStats>,
    sim_clock: Res<SimClock>,
    history: Res<PredictionHistory>,
    diagnostics: Res<DiagnosticsStore>,
    time: Res<Time>,
    mut debug_ui: ResMut<DebugUiState>,
    mut render_debug: ResMut<RenderDebugSettings>,
    mut player_tex_debug: ResMut<PlayerTextureDebugSettings>,
    render_perf: Res<RenderPerfStats>,
    sim_state: Res<SimState>,
    input: Res<CurrentInput>,
    player_status: Res<rs_utils::PlayerStatus>,
    collision_map: Res<WorldCollisionMap>,
    camera_query: Query<&GlobalTransform, With<PlayerCamera>>,
    mut timings: ResMut<PerfTimings>,
) {
    let timer = Timing::start();
    if !debug_ui.open {
        timings.debug_ui_ms = 0.0;
        return;
    }
    let ctx = contexts.ctx_mut().unwrap();
    egui::Window::new("Debug")
        .default_pos(egui::pos2(12.0, 12.0))
        .show(ctx, |ui| {
            let frame_ms = (time.delta_secs_f64() * 1000.0) as f32;
            let performance_section = egui::CollapsingHeader::new("Performance")
                .default_open(debug_ui.show_performance)
                .show(ui, |ui| {
                    ui.separator();
                    if let Some(fps) = diagnostics
                        .get(&FrameTimeDiagnosticsPlugin::FPS)
                        .and_then(|d| d.smoothed())
                    {
                        ui.label(format!("fps: {:.1}", fps));
                    } else {
                        ui.label("fps: n/a");
                    }
                    ui.label(format!("frame ms (delta): {:.2}", frame_ms));
                    ui.label(format!(
                        "main thread ms: {:.2} {}",
                        timings.main_thread_ms,
                        if frame_ms > 0.0 {
                            format!("{:.1}%", (timings.main_thread_ms / frame_ms) * 100.0)
                        } else {
                            "n/a".to_string()
                        }
                    ));
                });
            debug_ui.show_performance = performance_section.fully_open();

            let render_section = egui::CollapsingHeader::new("Render")
                .default_open(debug_ui.show_render)
                .show(ui, |ui| {
                    ui.separator();
                    let layers_section = egui::CollapsingHeader::new("Layers")
                        .default_open(debug_ui.render_show_layers)
                        .show(ui, |ui| {
                            ui.separator();
                            ui.checkbox(&mut render_debug.show_layer_entities, "Layer: entities");
                            ui.checkbox(
                                &mut render_debug.show_layer_chunks_opaque,
                                "Layer: chunks opaque",
                            );
                            ui.checkbox(
                                &mut render_debug.show_layer_chunks_cutout,
                                "Layer: chunks cutout",
                            );
                            ui.checkbox(
                                &mut render_debug.show_layer_chunks_transparent,
                                "Layer: chunks transparent",
                            );
                            ui.checkbox(
                                &mut render_debug.manual_frustum_cull,
                                "Manual frustum cull",
                            );
                        });
                    debug_ui.render_show_layers = layers_section.fully_open();

                    let lighting_section = egui::CollapsingHeader::new("Lighting")
                        .default_open(debug_ui.render_show_lighting)
                        .show(ui, |ui| {
                            ui.separator();
                            ui.checkbox(
                                &mut render_debug.enable_pbr_terrain_lighting,
                                "Enable PBR path",
                            );
                            ui.checkbox(&mut render_debug.shadows_enabled, "Shadows");
                            ui.add(
                                egui::Slider::new(&mut render_debug.shader_quality_mode, 0..=3)
                                    .text("Shader quality mode"),
                            );
                            ui.add(
                                egui::Slider::new(
                                    &mut render_debug.shadow_distance_scale,
                                    0.25..=20.0,
                                )
                                .logarithmic(true)
                                .text("Shadow distance"),
                            );
                            ui.add(
                                egui::Slider::new(&mut render_debug.shadow_map_size, 256..=4096)
                                    .text("Shadow map size"),
                            );
                            ui.add(
                                egui::Slider::new(&mut render_debug.shadow_cascades, 1..=4)
                                    .text("Shadow cascades"),
                            );
                            ui.add(
                                egui::Slider::new(
                                    &mut render_debug.shadow_max_distance,
                                    8.0..=400.0,
                                )
                                .text("Shadow max distance"),
                            );
                            ui.add(
                                egui::Slider::new(
                                    &mut render_debug.shadow_first_cascade_far_bound,
                                    4.0..=200.0,
                                )
                                .text("Shadow first cascade far"),
                            );
                            ui.add(
                                egui::Slider::new(&mut render_debug.shadow_depth_bias, 0.0..=0.2)
                                    .text("Shadow depth bias"),
                            );
                            ui.add(
                                egui::Slider::new(&mut render_debug.shadow_normal_bias, 0.0..=2.0)
                                    .text("Shadow normal bias"),
                            );
                            ui.add(
                                egui::Slider::new(
                                    &mut render_debug.sun_azimuth_deg,
                                    -180.0..=180.0,
                                )
                                .text("Sun azimuth"),
                            );
                            ui.add(
                                egui::Slider::new(
                                    &mut render_debug.sun_elevation_deg,
                                    -20.0..=89.0,
                                )
                                .text("Sun elevation"),
                            );
                            ui.add(
                                egui::Slider::new(&mut render_debug.sun_strength, 0.0..=2.0)
                                    .text("Sun strength"),
                            );
                            ui.add(
                                egui::Slider::new(&mut render_debug.ambient_strength, 0.0..=2.0)
                                    .text("Ambient strength"),
                            );
                            ui.add(
                                egui::Slider::new(&mut render_debug.ambient_brightness, 0.0..=2.0)
                                    .text("Ambient brightness"),
                            );
                            ui.add(
                                egui::Slider::new(&mut render_debug.fog_density, 0.0..=0.08)
                                    .text("Fog density"),
                            );
                            ui.add(
                                egui::Slider::new(&mut render_debug.fog_start, 0.0..=500.0)
                                    .text("Fog start"),
                            );
                            ui.add(
                                egui::Slider::new(&mut render_debug.fog_end, 1.0..=700.0)
                                    .text("Fog end"),
                            );
                            ui.add(
                                egui::Slider::new(&mut render_debug.water_absorption, 0.0..=1.0)
                                    .text("Water absorption"),
                            );
                            ui.add(
                                egui::Slider::new(&mut render_debug.water_fresnel, 0.0..=1.0)
                                    .text("Water fresnel"),
                            );
                        });
                    debug_ui.render_show_lighting = lighting_section.fully_open();

                    let water_section = egui::CollapsingHeader::new("Water")
                        .default_open(debug_ui.render_show_water)
                        .show(ui, |ui| {
                            ui.separator();
                            ui.checkbox(
                                &mut render_debug.water_reflections_enabled,
                                "Water reflections",
                            );
                            ui.checkbox(
                                &mut render_debug.water_reflection_screen_space,
                                "Screen-space SSR raymarch",
                            );
                            if render_debug.water_reflection_screen_space {
                                ui.add(
                                    egui::Slider::new(&mut render_debug.water_ssr_steps, 4..=64)
                                        .text("SSR ray steps"),
                                );
                                ui.add(
                                    egui::Slider::new(
                                        &mut render_debug.water_ssr_thickness,
                                        0.02..=2.0,
                                    )
                                    .text("SSR hit thickness"),
                                );
                                ui.add(
                                    egui::Slider::new(
                                        &mut render_debug.water_ssr_max_distance,
                                        4.0..=400.0,
                                    )
                                    .text("SSR max distance"),
                                );
                                ui.add(
                                    egui::Slider::new(
                                        &mut render_debug.water_ssr_stride,
                                        0.2..=8.0,
                                    )
                                    .text("SSR step stride"),
                                );
                            }
                            ui.add(
                                egui::Slider::new(
                                    &mut render_debug.water_reflection_strength,
                                    0.0..=3.0,
                                )
                                .text("Water reflection strength"),
                            );
                            ui.add(
                                egui::Slider::new(
                                    &mut render_debug.water_reflection_near_boost,
                                    0.0..=1.0,
                                )
                                .text("Near reflection boost"),
                            );
                            ui.checkbox(
                                &mut render_debug.water_reflection_blue_tint,
                                "Blue reflection tint",
                            );
                            ui.add(
                                egui::Slider::new(
                                    &mut render_debug.water_reflection_tint_strength,
                                    0.0..=2.0,
                                )
                                .text("Blue tint strength"),
                            );
                            ui.add(
                                egui::Slider::new(&mut render_debug.water_wave_strength, 0.0..=1.2)
                                    .text("Water wave strength"),
                            );
                            ui.add(
                                egui::Slider::new(&mut render_debug.water_wave_speed, 0.0..=3.0)
                                    .text("Water wave speed"),
                            );
                            ui.add(
                                egui::Slider::new(
                                    &mut render_debug.water_wave_detail_strength,
                                    0.0..=1.2,
                                )
                                .text("Wave detail strength"),
                            );
                            ui.add(
                                egui::Slider::new(
                                    &mut render_debug.water_wave_detail_scale,
                                    1.0..=8.0,
                                )
                                .text("Wave detail scale"),
                            );
                            ui.add(
                                egui::Slider::new(
                                    &mut render_debug.water_wave_detail_speed,
                                    0.0..=4.0,
                                )
                                .text("Wave detail speed"),
                            );
                            ui.add(
                                egui::Slider::new(
                                    &mut render_debug.water_reflection_edge_fade,
                                    0.02..=0.5,
                                )
                                .text("Reflection edge fade"),
                            );
                            ui.add(
                                egui::Slider::new(
                                    &mut render_debug.water_reflection_sky_fill,
                                    0.0..=1.0,
                                )
                                .text("Sky fallback fill"),
                            );
                            ui.add(
                                egui::Slider::new(
                                    &mut render_debug.water_reflection_overscan,
                                    1.0..=3.0,
                                )
                                .text("Reflection overscan"),
                            );
                        });
                    debug_ui.render_show_water = water_section.fully_open();

                    let misc_section = egui::CollapsingHeader::new("Misc")
                        .default_open(debug_ui.render_show_misc)
                        .show(ui, |ui| {
                            ui.separator();
                            let mut aa_mode = render_debug.aa_mode;
                            egui::ComboBox::from_label("AA")
                                .selected_text(aa_mode.label())
                                .show_ui(ui, |ui| {
                                    for mode in rs_render::AntiAliasingMode::ALL {
                                        ui.selectable_value(&mut aa_mode, mode, mode.label());
                                    }
                                });
                            if aa_mode != render_debug.aa_mode {
                                render_debug.aa_mode = aa_mode;
                                render_debug.fxaa_enabled = matches!(
                                    render_debug.aa_mode,
                                    rs_render::AntiAliasingMode::Fxaa
                                        | rs_render::AntiAliasingMode::Msaa4
                                        | rs_render::AntiAliasingMode::Msaa8
                                );
                            }
                            ui.checkbox(
                                &mut render_debug.use_greedy_meshing,
                                "Binary greedy meshing",
                            );
                            ui.checkbox(&mut render_debug.wireframe_enabled, "Wireframe");
                            ui.checkbox(&mut render_debug.voxel_ao_enabled, "Voxel AO");
                            ui.checkbox(&mut render_debug.voxel_ao_cutout, "Voxel AO on cutout");
                            ui.add(
                                egui::Slider::new(&mut render_debug.voxel_ao_strength, 0.0..=1.0)
                                    .text("Voxel AO strength"),
                            );
                            if ui.button("Force remesh chunks").clicked() {
                                render_debug.force_remesh = true;
                            }
                            if ui.button("Reset Debug/Render Settings").clicked() {
                                *render_debug = RenderDebugSettings::default();
                            }
                            if ui.button("Rebuild render materials").clicked() {
                                render_debug.material_rebuild_nonce =
                                    render_debug.material_rebuild_nonce.wrapping_add(1);
                            }
                            ui.checkbox(&mut render_debug.render_held_items, "Render held items");
                            ui.checkbox(
                                &mut render_debug.render_first_person_arms,
                                "First-person arms",
                            );
                            ui.checkbox(&mut render_debug.render_self_model, "Render self model");
                            ui.checkbox(&mut render_debug.show_chunk_borders, "Chunk borders");
                            ui.checkbox(&mut render_debug.show_coordinates, "Coordinates");
                            ui.checkbox(&mut render_debug.show_look_info, "Look info");
                            ui.checkbox(&mut render_debug.show_look_ray, "Look ray");
                            ui.checkbox(
                                &mut render_debug.show_target_block_outline,
                                "Target block outline",
                            );
                        });
                    debug_ui.render_show_misc = misc_section.fully_open();
                    let mut cutout_mode = render_debug.cutout_debug_mode as i32;
                    egui::ComboBox::from_label("Cutout debug")
                        .selected_text(match cutout_mode {
                            1 => "Pass id",
                            2 => "Atlas RGB",
                            3 => "Atlas alpha",
                            4 => "Vertex tint",
                            5 => "Linear depth",
                            6 => "Pass flags",
                            7 => "Alpha + pass",
                            8 => "Cutout lit flags",
                            _ => "Off",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut cutout_mode, 0, "Off");
                            ui.selectable_value(&mut cutout_mode, 1, "Pass id");
                            ui.selectable_value(&mut cutout_mode, 2, "Atlas rgb");
                            ui.selectable_value(&mut cutout_mode, 3, "Atlas alpha");
                            ui.selectable_value(&mut cutout_mode, 4, "Vertex tint");
                            ui.selectable_value(&mut cutout_mode, 5, "Linear depth");
                            ui.selectable_value(&mut cutout_mode, 6, "Pass flags");
                            ui.selectable_value(&mut cutout_mode, 7, "Alpha + pass");
                            ui.selectable_value(&mut cutout_mode, 8, "Cutout lit flags");
                        });
                    render_debug.cutout_debug_mode = cutout_mode.clamp(0, 8) as u8;
                    ui.checkbox(&mut render_debug.frustum_fov_debug, "Frustum FOV debug");
                    ui.checkbox(&mut player_tex_debug.flip_u, "Flip player skin U");
                    ui.checkbox(&mut player_tex_debug.flip_v, "Flip player skin V");
                    if render_debug.frustum_fov_debug {
                        ui.add(
                            egui::Slider::new(&mut render_debug.frustum_fov_deg, 30.0..=140.0)
                                .text("Frustum FOV"),
                        );
                    }
                    let mut dist = render_debug.render_distance_chunks as i32;
                    if ui
                        .add(egui::Slider::new(&mut dist, 2..=32).text("Render distance"))
                        .changed()
                    {
                        render_debug.render_distance_chunks = dist;
                    }
                    ui.add(egui::Slider::new(&mut render_debug.fov_deg, 60.0..=140.0).text("FOV"));

                    if render_debug.show_coordinates || render_debug.show_look_info {
                        ui.separator();
                    }
                    if render_debug.show_coordinates {
                        let pos = sim_state.current.pos;
                        let block = pos.floor().as_ivec3();
                        let chunk_x = block.x.div_euclid(16);
                        let chunk_z = block.z.div_euclid(16);
                        ui.label(format!("pos: {:.3} {:.3} {:.3}", pos.x, pos.y, pos.z));
                        ui.label(format!("block: {} {} {}", block.x, block.y, block.z));
                        ui.label(format!("chunk: {} {}", chunk_x, chunk_z));
                    }
                    if render_debug.show_look_info {
                        let yaw_mc = (std::f32::consts::PI - input.0.yaw).to_degrees();
                        let pitch_mc = (-input.0.pitch).to_degrees();
                        let (card, axis) = yaw_deg_to_cardinal(yaw_mc);
                        ui.label(format!("yaw/pitch: {:.1} / {:.1}", yaw_mc, pitch_mc));
                        ui.label(format!("facing: {} ({})", card, axis));

                        if let Ok(camera_transform) = camera_query.get_single() {
                            let origin = camera_transform.translation();
                            let dir = *camera_transform.forward();
                            let max_reach = if player_status.gamemode == 1 {
                                CREATIVE_BLOCK_REACH
                            } else {
                                SURVIVAL_BLOCK_REACH
                            };
                            if let Some(hit) = raycast_block(&collision_map, origin, dir, max_reach)
                            {
                                let state =
                                    collision_map.block_at(hit.block.x, hit.block.y, hit.block.z);
                                let id = block_state_id(state);
                                let meta = block_state_meta(state);
                                let kind = block_model_kind(id);
                                let reg = block_registry_key(id).unwrap_or("minecraft:unknown");
                                let world = WorldCollision::with_map(&collision_map);
                                let boxes = debug_block_collision_boxes(
                                    &world,
                                    state,
                                    hit.block.x,
                                    hit.block.y,
                                    hit.block.z,
                                );

                                ui.label(format!(
                                    "target block: {} {} {}",
                                    hit.block.x, hit.block.y, hit.block.z
                                ));
                                ui.label(format!(
                                    "id/state/meta: {} / {} / {}  kind: {}",
                                    id,
                                    state,
                                    meta,
                                    block_model_kind_label(kind)
                                ));
                                ui.label(format!("registry: {}", reg));
                                ui.label(format!("collision boxes: {}", boxes.len()));
                                for (idx, (min, max)) in boxes.iter().take(4).enumerate() {
                                    let size = *max - *min;
                                    ui.label(format!(
                                        "box{} min({:.3},{:.3},{:.3}) size({:.3},{:.3},{:.3})",
                                        idx, min.x, min.y, min.z, size.x, size.y, size.z
                                    ));
                                }
                                if boxes.len() > 4 {
                                    ui.label(format!("... {} more boxes", boxes.len() - 4));
                                }
                            } else {
                                ui.label("target block: none");
                            }
                        }
                    }
                });
            debug_ui.show_render = render_section.fully_open();

            let prediction_section = egui::CollapsingHeader::new("Prediction")
                .default_open(debug_ui.show_prediction)
                .show(ui, |ui| {
                    ui.separator();
                    ui.label(format!("tick: {}", sim_clock.tick));
                    ui.label(format!("history cap: {}", history.0.capacity()));
                    ui.label(format!("last correction: {:.4}", debug.last_correction));
                    ui.label(format!("last replay ticks: {}", debug.last_replay));
                    ui.label(format!(
                        "smoothing offset: {:.4}",
                        debug.smoothing_offset_len
                    ));
                    ui.label(format!("one-way ticks: {}", debug.one_way_ticks));
                });
            debug_ui.show_prediction = prediction_section.fully_open();

            if debug_ui.show_performance {
                ui.separator();
                let schedule_section = egui::CollapsingHeader::new("Schedule Timings")
                    .default_open(debug_ui.perf_show_schedules)
                    .show(ui, |_ui| {});
                debug_ui.perf_show_schedules = schedule_section.fully_open();
                let render_stats_section = egui::CollapsingHeader::new("Render Timings")
                    .default_open(debug_ui.perf_show_render_stats)
                    .show(ui, |_ui| {});
                debug_ui.perf_show_render_stats = render_stats_section.fully_open();
                let pct = |ms: f32| {
                    if frame_ms <= 0.0 {
                        None
                    } else {
                        Some((ms / frame_ms).max(0.0) * 100.0)
                    }
                };
                let fmt_pct = |ms: f32| {
                    pct(ms)
                        .map(|p| format!("{:.1}%", p))
                        .unwrap_or_else(|| "n/a".to_string())
                };
                if debug_ui.perf_show_schedules {
                    ui.label(format!(
                        "handle_messages: {:.3}ms {}",
                        timings.handle_messages_ms,
                        fmt_pct(timings.handle_messages_ms)
                    ));
                    ui.label(format!(
                        "update schedule: {:.3}ms {}",
                        timings.update_ms,
                        fmt_pct(timings.update_ms)
                    ));
                    ui.label(format!(
                        "post update: {:.3}ms {}",
                        timings.post_update_ms,
                        fmt_pct(timings.post_update_ms)
                    ));
                    ui.label(format!(
                        "fixed update: {:.3}ms {}",
                        timings.fixed_update_ms,
                        fmt_pct(timings.fixed_update_ms)
                    ));
                    ui.label(format!(
                        "input_collect: {:.3}ms {}",
                        timings.input_collect_ms,
                        fmt_pct(timings.input_collect_ms)
                    ));
                    ui.label(format!(
                        "net_apply: {:.3}ms {}",
                        timings.net_apply_ms,
                        fmt_pct(timings.net_apply_ms)
                    ));
                    ui.label(format!(
                        "fixed_tick: {:.3}ms {}",
                        timings.fixed_tick_ms,
                        fmt_pct(timings.fixed_tick_ms)
                    ));
                    ui.label(format!(
                        "smoothing: {:.3}ms {}",
                        timings.smoothing_ms,
                        fmt_pct(timings.smoothing_ms)
                    ));
                    ui.label(format!(
                        "apply_transform: {:.3}ms {}",
                        timings.apply_transform_ms,
                        fmt_pct(timings.apply_transform_ms)
                    ));
                    ui.label(format!(
                        "debug_ui: {:.3}ms {}",
                        timings.debug_ui_ms,
                        fmt_pct(timings.debug_ui_ms)
                    ));
                    ui.label(format!(
                        "ui: {:.3}ms {}",
                        timings.ui_ms,
                        fmt_pct(timings.ui_ms)
                    ));
                }
                if debug_ui.perf_show_render_stats {
                    ui.separator();
                    ui.label(format!(
                        "mesh build ms: {:.2} (avg {:.2}) [async]",
                        render_perf.last_mesh_build_ms, render_perf.avg_mesh_build_ms
                    ));
                    ui.label(format!(
                        "mesh apply ms: {:.2} (avg {:.2}) {}",
                        render_perf.last_apply_ms,
                        render_perf.avg_apply_ms,
                        fmt_pct(render_perf.last_apply_ms)
                    ));
                    ui.label(format!(
                        "mesh enqueue ms: {:.2} (avg {:.2}) {}",
                        render_perf.last_enqueue_ms,
                        render_perf.avg_enqueue_ms,
                        fmt_pct(render_perf.last_enqueue_ms)
                    ));
                    ui.label(format!(
                        "occlusion cull ms: {:.2} {}",
                        render_perf.occlusion_cull_ms,
                        fmt_pct(render_perf.occlusion_cull_ms)
                    ));
                    ui.label(format!(
                        "render debug ms: {:.2} {}",
                        render_perf.apply_debug_ms,
                        fmt_pct(render_perf.apply_debug_ms)
                    ));
                    ui.label(format!(
                        "render stats ms: {:.2} {}",
                        render_perf.gather_stats_ms,
                        fmt_pct(render_perf.gather_stats_ms)
                    ));
                    ui.label(format!(
                        "mesh applied: {} in_flight: {} updates: {} (raw {})",
                        render_perf.last_meshes_applied,
                        render_perf.in_flight,
                        render_perf.last_updates,
                        render_perf.last_updates_raw
                    ));
                    ui.label(format!(
                        "meshes: dist {} / {} view {} / {}",
                        render_perf.visible_meshes_distance,
                        render_perf.total_meshes,
                        render_perf.visible_meshes_view,
                        render_perf.total_meshes
                    ));
                    ui.label(format!(
                        "chunks: {} / {} (distance)",
                        render_perf.visible_chunks, render_perf.total_chunks
                    ));
                    ui.label(format!(
                        "chunks after occlusion: {} (occluded {})",
                        render_perf.visible_chunks_after_occlusion,
                        render_perf.occluded_chunks
                    ));
                    ui.separator();
                    ui.label(format!(
                        "mat pass w: o={:.1} c={:.1} cc={:.1} t={:.1}",
                        render_perf.mat_pass_opaque,
                        render_perf.mat_pass_cutout,
                        render_perf.mat_pass_cutout_culled,
                        render_perf.mat_pass_transparent
                    ));
                    ui.label(format!(
                        "mat alpha: o={} c={} cc={} t={}",
                        render_perf.mat_alpha_opaque,
                        render_perf.mat_alpha_cutout,
                        render_perf.mat_alpha_cutout_culled,
                        render_perf.mat_alpha_transparent
                    ));
                    ui.label(format!(
                        "mat unlit: o={} c={} cc={} t={}",
                        render_perf.mat_unlit_opaque,
                        render_perf.mat_unlit_cutout,
                        render_perf.mat_unlit_cutout_culled,
                        render_perf.mat_unlit_transparent
                    ));
                }
            }
        });

    let elapsed_ms = timer.ms();
    timings.debug_ui_ms = elapsed_ms;
}
