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
use crate::sim::movement::{WorldCollision, effective_sprint, simulate_tick};
use crate::sim::predict::PredictionBuffer;
use crate::sim::reconcile::reconcile;
use crate::sim::{
    CameraPerspectiveAltHold, CameraPerspectiveMode, CameraPerspectiveState, CurrentInput,
    DebugStats, DebugUiState, LocalArmSwing, PredictedFrame, SimClock, SimRenderState, SimState,
    VisualCorrectionOffset, ZoomState,
};
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use rs_render::{RenderDebugSettings, debug::RenderPerfStats};
use rs_utils::{BreakIndicator, EntityUseAction, InventoryState, PerfTimings};

use crate::entities::RemoteEntity;
use crate::entities::{ItemSpriteStack, PlayerTextureDebugSettings, RemoteVisual};
use crate::item_textures::{ItemSpriteMesh, ItemTextureCache};

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

pub fn input_collect_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut motion_events: EventReader<MouseMotion>,
    mut input: ResMut<CurrentInput>,
    app_state: Res<AppState>,
    ui_state: Res<UiState>,
    player_status: Res<rs_utils::PlayerStatus>,
    mut timings: ResMut<PerfTimings>,
) {
    let start = std::time::Instant::now();
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
        timings.input_collect_ms = start.elapsed().as_secs_f32() * 1000.0;
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

    input.0.jump = keys.pressed(KeyCode::Space);

    input.0.sprint = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    input.0.sneak = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    timings.input_collect_ms = start.elapsed().as_secs_f32() * 1000.0;
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

    item_textures.request_stack(stack);
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
) {
    if ui_state.chat_open || ui_state.inventory_open {
        return;
    }

    if !keys.just_pressed(KeyCode::F5) {
        return;
    }

    perspective.mode = match perspective.mode {
        CameraPerspectiveMode::FirstPerson => CameraPerspectiveMode::ThirdPersonBack,
        CameraPerspectiveMode::ThirdPersonBack => CameraPerspectiveMode::ThirdPersonFront,
        CameraPerspectiveMode::ThirdPersonFront => CameraPerspectiveMode::FirstPerson,
    };
}

pub fn camera_perspective_alt_hold_system(
    keys: Res<ButtonInput<KeyCode>>,
    ui_state: Res<UiState>,
    mut perspective: ResMut<CameraPerspectiveState>,
    mut alt_hold: ResMut<CameraPerspectiveAltHold>,
) {
    if ui_state.chat_open || ui_state.inventory_open {
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
    collision_map: Res<WorldCollisionMap>,
    app_state: Res<AppState>,
    to_net: Res<ToNet>,
    sim_ready: Res<crate::sim::SimReady>,
    mut timings: ResMut<PerfTimings>,
) {
    let start = std::time::Instant::now();
    if !matches!(app_state.0, ApplicationState::Connected) || !sim_ready.0 {
        timings.fixed_tick_ms = start.elapsed().as_secs_f32() * 1000.0;
        return;
    }
    let world = WorldCollision::with_map(&collision_map);
    let tick = sim_clock.tick;
    let input_snapshot = input.0;
    sim_render.previous = sim_state.current;
    let next_state = simulate_tick(&sim_state.current, &input_snapshot, &world);

    history.0.push(PredictedFrame {
        tick,
        input: input_snapshot,
        state: next_state,
    });

    sim_state.current = next_state;
    sim_clock.tick = sim_clock.tick.wrapping_add(1);
    input.0.jump = false;

    if matches!(app_state.0, ApplicationState::Connected) {
        let current_sneak = input_snapshot.sneak;
        let current_sprint = effective_sprint(&input_snapshot);

        if current_sneak != action_state.sneaking {
            let action_id = if current_sneak { 0 } else { 1 };
            let _ = to_net.0.send(ToNetMessage::PlayerAction { action_id });
            action_state.sneaking = current_sneak;
        }
        if current_sprint != action_state.sprinting {
            let action_id = if current_sprint { 3 } else { 4 };
            let _ = to_net.0.send(ToNetMessage::PlayerAction { action_id });
            action_state.sprinting = current_sprint;
        }

        let pos = sim_state.current.pos;
        let yaw = (std::f32::consts::PI - sim_state.current.yaw).to_degrees();
        let pitch = -sim_state.current.pitch.to_degrees();
        let _ = to_net.0.send(ToNetMessage::PlayerMove {
            x: pos.x as f64,
            y: pos.y as f64,
            z: pos.z as f64,
            yaw,
            pitch,
            on_ground: sim_state.current.on_ground,
        });
        latency.last_sent = Some(Instant::now());
    }
    timings.fixed_tick_ms = start.elapsed().as_secs_f32() * 1000.0;
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
    collision_map: Res<WorldCollisionMap>,
    sim_clock: Res<SimClock>,
    mut sim_ready: ResMut<crate::sim::SimReady>,
    mut timings: ResMut<PerfTimings>,
) {
    let start = std::time::Instant::now();
    let world = WorldCollision::with_map(&collision_map);
    const FORCE_TELEPORT_DISTANCE: f32 = 8.0;
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

        let client_tick = sim_clock.tick;
        let server_tick = client_tick.saturating_sub(latency.one_way_ticks);

        let server_state = crate::sim::PlayerSimState {
            pos,
            vel: sim_state.current.vel,
            on_ground,
            yaw,
            pitch,
        };

        // Large server position jumps (respawn/teleport) should snap immediately.
        if (server_state.pos - sim_state.current.pos).length() >= FORCE_TELEPORT_DISTANCE {
            sim_render.previous = server_state;
            sim_state.current = server_state;
            history.0 = PredictionHistory::default().0;
            visual_offset.0 = Vec3::ZERO;
            sim_ready.0 = true;
            debug.last_correction = 0.0;
            debug.last_replay = 0;
            continue;
        }

        if history.0.latest_tick().is_none() {
            sim_render.previous = server_state;
            sim_state.current = server_state;
            visual_offset.0 = Vec3::ZERO;
            sim_ready.0 = true;
            continue;
        }

        if let Some(result) = reconcile(
            &mut history.0,
            &world,
            server_tick,
            server_state,
            client_tick.saturating_sub(1),
            &mut sim_state.current,
        ) {
            sim_render.previous = sim_state.current;
            debug.last_correction = result.correction.length();
            debug.last_replay = result.replayed_ticks;
            if result.hard_teleport {
                visual_offset.0 = Vec3::ZERO;
            } else {
                visual_offset.0 += result.correction;
            }
        }
    }
    timings.net_apply_ms = start.elapsed().as_secs_f32() * 1000.0;
}

pub fn visual_smoothing_system(
    time: Res<Time>,
    mut offset: ResMut<VisualCorrectionOffset>,
    mut debug: ResMut<DebugStats>,
    mut timings: ResMut<PerfTimings>,
) {
    let start = std::time::Instant::now();
    let decay = 0.15f32;
    let factor = (1.0 - decay).powf(time.delta_secs() * 20.0);
    offset.0 *= factor;
    debug.smoothing_offset_len = offset.0.length();
    timings.smoothing_ms = start.elapsed().as_secs_f32() * 1000.0;
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
    let start = std::time::Instant::now();
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
    timings.apply_transform_ms = start.elapsed().as_secs_f32() * 1000.0;
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
    inventory_state: Res<InventoryState>,
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
    let block_hit = raycast_block(&collision_map, origin, dir, 6.0);
    let entity_hit = raycast_remote_entity(&remote_entities, origin, dir, 4.5);

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
        } else {
            let held_item = inventory_state.hotbar_item(inventory_state.selected_hotbar_slot);
            let _ = to_net.0.send(ToNetMessage::UseItem { held_item });
        }
    }
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
    if hardness <= 0.0 {
        return 0.05;
    }
    let tool_mult = held_item
        .map(|stack| tool_efficiency_multiplier(stack.item_id, block_id))
        .unwrap_or(1.0);
    let secs = (hardness * 1.5) / tool_mult.max(0.1);
    secs.clamp(0.1, 10.0)
}

fn block_hardness(block_id: u16) -> f32 {
    match block_id {
        0 => 0.0,            // air
        1 | 4 => 2.0,        // stone, cobble
        2 => 0.6,            // grass
        3 => 0.5,            // dirt
        5 | 17 => 2.0,       // planks/log
        12 => 0.5,           // sand
        13 => 0.6,           // gravel
        14 | 15 | 16 => 3.0, // ores
        18 => 0.2,           // leaves
        20 => 0.3,           // glass
        24 | 45 => 0.8,      // sandstone, brick
        49 => 50.0,          // obsidian
        50 => 0.0,           // torch
        54 => 2.5,           // chest
        58 => 2.5,           // crafting
        61 | 62 => 3.5,      // furnace
        79 => 0.5,           // ice
        80 => 0.2,           // snow block
        81 => 0.4,           // cactus
        82 => 0.6,           // clay
        87 => 0.4,           // netherrack
        88 => 0.5,           // soulsand
        89 => 0.3,           // glowstone
        _ => 1.0,
    }
}

fn tool_efficiency_multiplier(item_id: i32, block_id: u16) -> f32 {
    let is_pickaxe = matches!(item_id, 257 | 270 | 274 | 278 | 285);
    let is_shovel = matches!(item_id, 256 | 269 | 273 | 277 | 284);
    let is_axe = matches!(item_id, 258 | 271 | 275 | 279 | 286);

    let tier_mult = match item_id {
        269 | 270 | 271 => 2.0, // wood
        273 | 274 | 275 => 4.0, // stone
        256..=258 => 6.0,       // iron
        277..=279 => 8.0,       // diamond
        284..=286 => 12.0,      // gold
        _ => 1.0,
    };

    let pick_blocks = matches!(
        block_id,
        1 | 4 | 14 | 15 | 16 | 21 | 22 | 24 | 41 | 42 | 45 | 49 | 56 | 57 | 61 | 62 | 73 | 74
    );
    let shovel_blocks = matches!(block_id, 2 | 3 | 12 | 13 | 80 | 82 | 88);
    let axe_blocks = matches!(block_id, 5 | 17 | 47 | 53 | 54 | 58);

    if (is_pickaxe && pick_blocks) || (is_shovel && shovel_blocks) || (is_axe && axe_blocks) {
        tier_mult
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
            let block_id = world.block_at(cell.x, cell.y, cell.z);
            if block_id != 0 {
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
    mut timings: ResMut<PerfTimings>,
) {
    let start = std::time::Instant::now();
    if !debug_ui.open {
        timings.debug_ui_ms = 0.0;
        return;
    }
    let ctx = contexts.ctx_mut().unwrap();
    egui::Window::new("Debug")
        .default_pos(egui::pos2(12.0, 12.0))
        .show(ctx, |ui| {
            ui.checkbox(&mut debug_ui.show_prediction, "Prediction");
            ui.checkbox(&mut debug_ui.show_performance, "Performance");
            ui.checkbox(&mut debug_ui.show_render, "Render");

            let frame_ms = (time.delta_secs_f64() * 1000.0) as f32;

            if debug_ui.show_performance {
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
            }

            if debug_ui.show_render {
                ui.separator();
                ui.checkbox(&mut render_debug.shadows_enabled, "Shadows");
                ui.checkbox(&mut render_debug.fxaa_enabled, "FXAA");
                ui.checkbox(
                    &mut render_debug.use_greedy_meshing,
                    "Binary greedy meshing",
                );
                ui.checkbox(&mut render_debug.wireframe_enabled, "Wireframe");
                ui.checkbox(&mut render_debug.manual_frustum_cull, "Manual frustum cull");
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
                }
            }

            if debug_ui.show_prediction {
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
            }

            if debug_ui.show_performance {
                ui.separator();
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
                    "manual cull ms: {:.2} {}",
                    render_perf.manual_cull_ms,
                    fmt_pct(render_perf.manual_cull_ms)
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
            }
        });
    let elapsed_ms = start.elapsed().as_secs_f32() * 1000.0;
    timings.debug_ui_ms = elapsed_ms;
}
