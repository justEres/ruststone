use std::time::Instant;

use bevy::input::mouse::MouseMotion;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::time::Fixed;
use bevy_egui::{EguiContexts, egui};
use rs_sound::{block_step_sound, emit_ui_sound, emit_world_sound};

use rs_render::{ChunkRoot, LookAngles, Player, PlayerCamera};
use rs_utils::{
    AppState, ApplicationState, SoundCategory, SoundEventQueue, ToNet, ToNetMessage, UiState,
};

use crate::net::events::NetEventQueue;
use crate::sim::collision::WorldCollisionMap;
use crate::sim::movement::{
    WorldCollision, debug_block_collision_boxes, effective_sprint, simulate_tick,
};
use crate::sim::predict::PredictionBuffer;
use crate::sim::reconcile::reconcile;
use crate::sim::{
    CameraPerspectiveAltHold, CameraPerspectiveMode, CameraPerspectiveState, CorrectionLoopGuard,
    CurrentInput, DebugStats, DebugUiState, FreecamState, LocalArmSwing, MovementPacketState,
    PredictedFrame, SimClock, SimRenderState, SimState, VisualCorrectionOffset, ZoomState,
};
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use rs_render::{RenderDebugSettings, debug::RenderPerfStats};
use rs_utils::{
    BreakIndicator, EntityUseAction, InventoryState, PerfTimings, block_model_kind,
    block_registry_key, block_state_id, block_state_meta,
};

use crate::entities::{ItemSpriteStack, RemoteVisual};
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

#[derive(Default, Resource)]
pub struct MovementSoundState {
    pub accumulated_ground_distance: f32,
    pub accumulated_swim_distance: f32,
    pub was_in_water: bool,
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
const SPRINT_FORWARD_THRESHOLD: f32 = 0.8;
const VANILLA_STEP_TRIGGER_DISTANCE: f32 = 1.0 / 0.6;

fn gameplay_input_allowed(
    app_state: &AppState,
    ui_state: &UiState,
    player_status: &rs_utils::PlayerStatus,
) -> bool {
    matches!(app_state.0, ApplicationState::Connected)
        && !ui_state.chat_open
        && !ui_state.paused
        && !ui_state.inventory_open
        && !player_status.dead
}

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

fn yaw_pitch_from_forward(forward: Vec3) -> (f32, f32) {
    let f = forward.normalize_or_zero();
    let yaw = (-f.x).atan2(-f.z);
    let pitch = f.y.clamp(-1.0, 1.0).asin().clamp(-1.54, 1.54);
    (yaw, pitch)
}

pub fn input_collect_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut motion_events: EventReader<MouseMotion>,
    mut input: ResMut<CurrentInput>,
    perspective: Res<CameraPerspectiveState>,
    freecam: Res<FreecamState>,
    app_state: Res<AppState>,
    ui_state: Res<UiState>,
    player_status: Res<rs_utils::PlayerStatus>,
    mut timings: ResMut<PerfTimings>,
) {
    let timer = Timing::start();
    if !gameplay_input_allowed(&app_state, &ui_state, &player_status) {
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

    if !freecam.active && matches!(perspective.mode, CameraPerspectiveMode::ThirdPersonFront) {
        // Front-facing third-person camera is rotated 180deg around the player.
        // Map movement to camera-relative controls for this view.
        input.0.forward = -input.0.forward;
        input.0.strafe = -input.0.strafe;
    }

    input.0.jump = keys.pressed(KeyCode::Space);

    input.0.sprint = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    input.0.sneak = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if freecam.active {
        // Keep look deltas for freecam, but never drive player simulation while detached.
        input.0.forward = 0.0;
        input.0.strafe = 0.0;
        input.0.jump = false;
        input.0.sprint = false;
        input.0.sneak = false;
    }
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

    let input_allowed = gameplay_input_allowed(&app_state, &ui_state, &player_status);

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
    collision_map: Res<WorldCollisionMap>,
    mut item_textures: ResMut<ItemTextureCache>,
    item_sprite_mesh: Res<ItemSpriteMesh>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    camera: Query<(Entity, &GlobalTransform), With<PlayerCamera>>,
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

    let Ok((cam_entity, cam_transform)) = camera.get_single() else {
        return;
    };

    let cam_pos = cam_transform.translation();
    let cam_rot = cam_transform.compute_transform().rotation;
    // Held item occupies the right-lower-forward camera space; hide it when intersecting solids.
    let held_item_probes = [
        Vec3::new(0.20, -0.24, -0.38),
        Vec3::new(0.36, -0.32, -0.56),
        Vec3::new(0.48, -0.38, -0.72),
        Vec3::new(0.58, -0.42, -0.86),
    ];
    let colliding_near_wall = held_item_probes.into_iter().any(|probe| {
        let world = cam_pos + cam_rot * probe;
        let cell = world.floor().as_ivec3();
        crate::sim::collision::is_solid(collision_map.block_at(cell.x, cell.y, cell.z))
    });
    if colliding_near_wall {
        for e in existing.iter() {
            commands.entity(e).despawn_recursive();
        }
        return;
    }

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

pub fn freecam_toggle_system(
    keys: Res<ButtonInput<KeyCode>>,
    ui_state: Res<UiState>,
    app_state: Res<AppState>,
    mut freecam: ResMut<FreecamState>,
    mut perspective: ResMut<CameraPerspectiveState>,
    mut alt_hold: ResMut<CameraPerspectiveAltHold>,
    mut input: ResMut<CurrentInput>,
    mut player_query: Query<(&GlobalTransform, &mut LookAngles), With<Player>>,
    camera_query: Query<&GlobalTransform, With<PlayerCamera>>,
) {
    if ui_state.chat_open || ui_state.inventory_open || ui_state.paused {
        return;
    }
    if !matches!(app_state.0, ApplicationState::Connected) || !keys.just_pressed(KeyCode::F4) {
        return;
    }

    let Ok(camera_gt) = camera_query.get_single() else {
        return;
    };
    let camera_forward = camera_gt.forward().as_vec3();
    let (yaw, pitch) = yaw_pitch_from_forward(camera_forward);
    perspective.mode = CameraPerspectiveMode::FirstPerson;
    alt_hold.saved_mode = None;

    if !freecam.active {
        freecam.active = true;
        freecam.position = camera_gt.translation();
        input.0.yaw = yaw;
        input.0.pitch = pitch;
        return;
    }

    freecam.active = false;
    if let Ok((_player_gt, mut look)) = player_query.get_single_mut() {
        look.yaw = yaw;
        look.pitch = pitch;
    }
    input.0.yaw = yaw;
    input.0.pitch = pitch;
}

pub fn freecam_move_system(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    input: Res<CurrentInput>,
    mut freecam: ResMut<FreecamState>,
    player_query: Query<&GlobalTransform, With<Player>>,
    mut camera_query: Query<&mut Transform, (With<PlayerCamera>, Without<Player>)>,
) {
    if !freecam.active {
        return;
    }

    let Ok(player_gt) = player_query.get_single() else {
        return;
    };
    let Ok(mut camera_local) = camera_query.get_single_mut() else {
        return;
    };

    let dt = time.delta_secs().clamp(0.0, 0.05);
    let yaw_rot = Quat::from_axis_angle(Vec3::Y, input.0.yaw);
    let pitch_rot = Quat::from_axis_angle(Vec3::X, input.0.pitch);
    let world_rot = yaw_rot * pitch_rot;
    let forward = world_rot * -Vec3::Z;
    let right = world_rot * Vec3::X;

    let mut move_forward = 0.0;
    let mut move_strafe = 0.0;
    if keys.pressed(KeyCode::KeyW) {
        move_forward += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        move_forward -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        move_strafe += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        move_strafe -= 1.0;
    }
    let vertical = if keys.pressed(KeyCode::Space) {
        1.0
    } else {
        0.0
    } - if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        1.0
    } else {
        0.0
    };

    let mut wish = forward * move_forward + right * move_strafe + Vec3::Y * vertical;
    if wish.length_squared() > 1.0 {
        wish = wish.normalize();
    }
    let sprint = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let speed = if sprint { 26.0 } else { 11.0 };
    freecam.position += wish * speed * dt;

    let player_world = player_gt.compute_transform();
    let inv_player_rot = player_world.rotation.inverse();
    camera_local.translation = inv_player_rot * (freecam.position - player_world.translation);
    camera_local.rotation = inv_player_rot * world_rot;
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

    // Vanilla 1.8 sprint latching parity:
    // - sprint start via sprint key requires strong forward input and sprint eligibility
    // - sprint cancel happens on weak forward input, horizontal collision, or low food
    let sprint_key_down = input_snapshot.sprint;
    let forward_strong = input_snapshot.forward >= SPRINT_FORWARD_THRESHOLD;
    let sprint_eligible = player_status.food > 6 || player_status.can_fly;
    let mut sprinting_state = action_state.sprinting;
    if !sprinting_state
        && sprint_key_down
        && forward_strong
        && sprint_eligible
        && !input_snapshot.sneak
    {
        sprinting_state = true;
    }
    if sprinting_state
        && (!forward_strong || sim_state.current.collided_horizontally || !sprint_eligible)
    {
        sprinting_state = false;
    }
    input_snapshot.sprint = sprinting_state;

    sim_render.previous = sim_state.current;
    let next_state = if correction_guard.skip_physics_ticks > 0 {
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

fn is_water_state(block_state: u16) -> bool {
    matches!(block_state_id(block_state), 8 | 9)
}

pub fn local_movement_sound_system(
    app_state: Res<AppState>,
    ui_state: Res<UiState>,
    player_status: Res<rs_utils::PlayerStatus>,
    sim_state: Res<SimState>,
    sim_render: Res<SimRenderState>,
    collision_map: Res<WorldCollisionMap>,
    mut movement_sound: ResMut<MovementSoundState>,
    mut sound_queue: ResMut<SoundEventQueue>,
) {
    if !gameplay_input_allowed(&app_state, &ui_state, &player_status) {
        movement_sound.accumulated_ground_distance = 0.0;
        movement_sound.accumulated_swim_distance = 0.0;
        movement_sound.was_in_water = false;
        return;
    }

    let previous = sim_render.previous;
    let current = sim_state.current;
    let horizontal_delta = Vec2::new(current.pos.x - previous.pos.x, current.pos.z - previous.pos.z);
    let horizontal_distance = horizontal_delta.length();
    if horizontal_distance <= f32::EPSILON {
        return;
    }

    let feet_block = collision_map.block_at(
        current.pos.x.floor() as i32,
        current.pos.y.floor() as i32,
        current.pos.z.floor() as i32,
    );
    let in_water = is_water_state(feet_block);

    if in_water && !movement_sound.was_in_water {
        emit_ui_sound(
            &mut sound_queue,
            "minecraft:random.splash",
            0.5,
            1.0,
        );
    }
    movement_sound.was_in_water = in_water;

    if in_water {
        movement_sound.accumulated_ground_distance = 0.0;
        movement_sound.accumulated_swim_distance += horizontal_distance;
        if movement_sound.accumulated_swim_distance >= VANILLA_STEP_TRIGGER_DISTANCE {
            emit_world_sound(
                &mut sound_queue,
                "minecraft:game.player.swim",
                current.pos,
                0.35,
                1.0,
                Some(SoundCategory::Player),
            );
            movement_sound.accumulated_swim_distance = 0.0;
        }
        return;
    }

    movement_sound.accumulated_swim_distance = 0.0;
    if !current.on_ground {
        movement_sound.accumulated_ground_distance = 0.0;
        return;
    }

    movement_sound.accumulated_ground_distance += horizontal_distance;
    if movement_sound.accumulated_ground_distance < VANILLA_STEP_TRIGGER_DISTANCE {
        return;
    }

    let below = Vec3::new(current.pos.x, current.pos.y - 0.2, current.pos.z).floor().as_ivec3();
    let ground_state = collision_map.block_at(below.x, below.y, below.z);
    emit_world_sound(
        &mut sound_queue,
        block_step_sound(block_state_id(ground_state)),
        current.pos,
        0.25,
        1.0,
        Some(SoundCategory::Block),
    );
    movement_sound.accumulated_ground_distance = 0.0;
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
    collision_map: Res<WorldCollisionMap>,
    sim_clock: Res<SimClock>,
    mut timings: ResMut<PerfTimings>,
) {
    let timer = Timing::start();
    let world = WorldCollision::with_map(&collision_map);
    for event in net_events.drain() {
        let (pos, yaw, pitch, on_ground, recv_instant) = match event {
            crate::net::events::NetEvent::ServerPosLook {
                pos,
                yaw,
                pitch,
                on_ground,
                recv_instant,
            } => (pos, yaw, pitch, on_ground, recv_instant),
            crate::net::events::NetEvent::ServerVelocity {
                velocity,
                recv_instant,
            } => {
                if let Some(last_sent) = latency.last_sent {
                    let rtt = recv_instant.saturating_duration_since(last_sent);
                    let one_way = rtt.as_secs_f32() * 0.5;
                    latency.one_way_ticks = (one_way / 0.05).round() as u32;
                    debug.one_way_ticks = latency.one_way_ticks;
                }

                let latest_tick = sim_clock.tick.saturating_sub(1);
                let server_tick = latest_tick.saturating_sub(latency.one_way_ticks);
                let mut authoritative_state = history
                    .0
                    .get_by_tick(server_tick)
                    .map(|frame| frame.state)
                    .or_else(|| history.0.latest_frame().map(|frame| frame.state))
                    .unwrap_or(sim_state.current);
                authoritative_state.vel = velocity;

                let previous_state = sim_state.current;
                if let Some(result) = reconcile(
                    &mut history.0,
                    &world,
                    server_tick,
                    authoritative_state,
                    latest_tick,
                    &mut sim_state.current,
                ) {
                    sim_render.previous = previous_state;
                    visual_offset.0 += previous_state.pos - sim_state.current.pos;
                    debug.last_correction = result.correction.length();
                    debug.last_replay = result.replayed_ticks;
                    debug.last_velocity_correction = result.velocity_correction;
                    debug.last_reconciled_server_tick = Some(server_tick);
                } else {
                    sim_state.current.vel = velocity;
                    if let Some(frame) = history.0.latest_frame_mut() {
                        frame.state = sim_state.current;
                    }
                    debug.last_velocity_correction = 0.0;
                    debug.last_reconciled_server_tick = Some(server_tick);
                }
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
            collided_horizontally: false,
            jump_ticks: 0,
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

        let latest_tick = sim_clock.tick.saturating_sub(1);
        let server_tick = latest_tick.saturating_sub(latency.one_way_ticks);
        let previous_state = sim_state.current;
        let reconcile_result = reconcile(
            &mut history.0,
            &world,
            server_tick,
            server_state,
            latest_tick,
            &mut sim_state.current,
        );

        if let Some(result) = reconcile_result {
            if result.hard_teleport {
                sim_render.previous = server_state;
                sim_state.current = server_state;
                history.0 = PredictionHistory::default().0;
                visual_offset.0 = Vec3::ZERO;
            } else {
                sim_render.previous = previous_state;
                visual_offset.0 += previous_state.pos - sim_state.current.pos;
            }
            debug.last_correction = result.correction.length();
            debug.last_replay = result.replayed_ticks;
            debug.last_velocity_correction = result.velocity_correction;
            debug.last_reconciled_server_tick = Some(server_tick);
        } else {
            sim_render.previous = sim_state.current;
            debug.last_correction = 0.0;
            debug.last_replay = 0;
            debug.last_velocity_correction = 0.0;
            debug.last_reconciled_server_tick = Some(server_tick);
        }
        sim_ready.0 = true;
        move_pkt_state.initialized = true;
        move_pkt_state.last_pos = sim_state.current.pos;
        move_pkt_state.last_yaw_deg = ack_yaw_deg;
        move_pkt_state.last_pitch_deg = ack_pitch_deg;
        move_pkt_state.ticks_since_pos = 0;
        correction_guard.skip_send_ticks = 0;
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
    freecam: Res<FreecamState>,
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
        if !freecam.active {
            look.yaw = input.0.yaw;
            look.pitch = input.0.pitch;
        }
        player_transform.rotation = Quat::from_axis_angle(Vec3::Y, look.yaw);
        if let Ok(mut camera_transform) = camera_query.get_single_mut() {
            if freecam.active {
                timings.apply_transform_ms = timer.ms();
                return;
            }
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

#[path = "sim_interaction/mod.rs"]
mod sim_interaction;

pub use sim_interaction::{
    debug_overlay_system, draw_chunk_debug_system, draw_entity_hitboxes_system,
    world_interaction_system,
};
