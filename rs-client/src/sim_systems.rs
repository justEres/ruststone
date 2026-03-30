use std::time::Instant;

use bevy::ecs::system::SystemParam;
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

use crate::sim::collision::WorldCollisionMap;
use crate::sim::movement::{
    WorldCollision, debug_block_collision_boxes, effective_sprint, simulate_tick,
};
use crate::sim::predict::PredictionBuffer;
use crate::sim::{
    CameraPerspectiveAltHold, CameraPerspectiveMode, CameraPerspectiveState, CurrentInput,
    DebugStats, DebugUiState, FreecamState, LocalArmSwing, PredictedFrame, SimClock,
    SimRenderState, SimState, VisualCorrectionOffset, ZoomState,
};
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use rs_render::{RenderDebugSettings, debug::RenderPerfStats};
use rs_utils::{
    BreakIndicator, EntityUseAction, InventoryState, PerfTimings, block_model_kind,
    block_registry_key, block_state_id, block_state_meta,
};
use sysinfo::{Pid, ProcessesToUpdate, System};

use crate::entities::{ItemSpriteStack, RemoteVisual};
use crate::entities::{RemoteEntity, RemoteEntityRegistry};
use crate::item_textures::{ItemSpriteMesh, ItemTextureCache};
use crate::movement_session::MovementSession;
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
    pub sent_sneaking: bool,
    pub sent_sprinting: bool,
    pub sim_sprinting: bool,
    pub jump_was_pressed: bool,
    pub fly_toggle_timer: u8,
}

#[derive(Default, Resource)]
pub struct MovementSoundState {
    pub accumulated_ground_distance: f32,
    pub accumulated_swim_distance: f32,
    pub was_in_water: bool,
}

#[derive(SystemParam)]
pub struct FixedSimParams<'w, 's> {
    pub render_debug: Res<'w, RenderDebugSettings>,
    pub to_net: Res<'w, ToNet>,
    pub remote_entities: Res<'w, RemoteEntityRegistry>,
    pub sim_ready: Res<'w, crate::sim::SimReady>,
    pub timings: ResMut<'w, PerfTimings>,
    pub _marker: std::marker::PhantomData<&'s ()>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PerfMonitorSample {
    pub frame_ms: f32,
    pub cpu_percent: f32,
    pub ram_mb: f32,
    pub render_ms: f32,
    pub render_mesh_upload_ms: f32,
    pub render_mesh_queue_ms: f32,
    pub render_occlusion_ms: f32,
    pub render_material_ms: f32,
    pub render_stats_ms: f32,
    pub render_breakdown_is_gpu: bool,
}

#[derive(Resource)]
pub struct PerformanceMonitorState {
    pub samples: std::collections::VecDeque<PerfMonitorSample>,
    pub pid: Option<Pid>,
    pub max_samples: usize,
}

impl Default for PerformanceMonitorState {
    fn default() -> Self {
        let pid = sysinfo::get_current_pid().ok();
        Self {
            samples: std::collections::VecDeque::with_capacity(240),
            pid,
            max_samples: 240,
        }
    }
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

fn effective_flying_speed(
    base_speed: f32,
    render_debug: &RenderDebugSettings,
    player_status: &rs_utils::PlayerStatus,
) -> f32 {
    if render_debug.flight_speed_boost_enabled && player_status.flying && player_status.can_fly {
        base_speed * render_debug.flight_speed_boost_multiplier.clamp(1.0, 10.0)
    } else {
        base_speed
    }
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

mod camera;
mod simulation;
mod timing;

pub use camera::{
    apply_visual_transform_system, camera_perspective_alt_hold_system,
    camera_perspective_toggle_system, debug_toggle_system, freecam_move_system,
    freecam_toggle_system,
};
pub use simulation::{
    fixed_sim_tick_system, local_arm_swing_tick_system, local_movement_sound_system,
    visual_smoothing_system,
};
pub use timing::{
    fixed_update_timing_end, fixed_update_timing_start, frame_timing_end, frame_timing_start,
    performance_monitor_sample_system, post_update_timing_end, post_update_timing_start,
    update_timing_end, update_timing_start,
};

#[path = "sim_interaction/mod.rs"]
mod sim_interaction;

pub use sim_interaction::{
    debug_overlay_system, draw_chunk_debug_system, draw_entity_hitboxes_system,
    world_interaction_system,
};
