use std::collections::HashMap;

use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use rs_utils::{BlockModelKind, block_model_kind, block_state_id};

use crate::{chunk, components, reflection};

const DYNAMIC_LIGHT_SCAN_RADIUS_XZ: i32 = 20;
const DYNAMIC_LIGHT_SCAN_RADIUS_Y: i32 = 12;
const DYNAMIC_LIGHT_MAX_COUNT: usize = 96;
const DYNAMIC_LIGHT_REFRESH_SECONDS: f32 = 0.20;
const DYNAMIC_LIGHT_INTENSITY_SCALE: f32 = 0.55;
const DYNAMIC_LIGHT_RANGE_SCALE: f32 = 0.80;

#[derive(Component)]
struct DynamicBlockLight;

#[derive(Resource)]
pub(super) struct DynamicBlockLightState {
    by_pos: HashMap<IVec3, Entity>,
    refresh_timer: Timer,
}

impl Default for DynamicBlockLightState {
    fn default() -> Self {
        Self {
            by_pos: HashMap::new(),
            refresh_timer: Timer::from_seconds(DYNAMIC_LIGHT_REFRESH_SECONDS, TimerMode::Repeating),
        }
    }
}

#[derive(Clone, Copy)]
struct BlockLightSpec {
    color: Color,
    intensity: f32,
    range: f32,
    y_offset: f32,
}

fn block_light_spec(block_state: u16) -> Option<BlockLightSpec> {
    let id = block_state_id(block_state);
    let spec = match id {
        50 => BlockLightSpec {
            color: Color::srgb(1.0, 0.76, 0.46),
            intensity: 700.0,
            range: 9.5,
            y_offset: 0.62,
        },
        76 => BlockLightSpec {
            color: Color::srgb(1.0, 0.20, 0.18),
            intensity: 230.0,
            range: 6.2,
            y_offset: 0.62,
        },
        51 => BlockLightSpec {
            color: Color::srgb(1.0, 0.57, 0.24),
            intensity: 860.0,
            range: 10.5,
            y_offset: 0.55,
        },
        10 | 11 => BlockLightSpec {
            color: Color::srgb(1.0, 0.42, 0.12),
            intensity: 980.0,
            range: 11.0,
            y_offset: 0.50,
        },
        62 => BlockLightSpec {
            color: Color::srgb(1.0, 0.62, 0.33),
            intensity: 450.0,
            range: 7.8,
            y_offset: 0.50,
        },
        74 => BlockLightSpec {
            color: Color::srgb(1.0, 0.22, 0.20),
            intensity: 250.0,
            range: 6.4,
            y_offset: 0.50,
        },
        89 => BlockLightSpec {
            color: Color::srgb(1.0, 0.94, 0.74),
            intensity: 1200.0,
            range: 12.8,
            y_offset: 0.50,
        },
        90 => BlockLightSpec {
            color: Color::srgb(0.72, 0.34, 1.0),
            intensity: 520.0,
            range: 8.2,
            y_offset: 0.50,
        },
        91 => BlockLightSpec {
            color: Color::srgb(1.0, 0.88, 0.56),
            intensity: 1020.0,
            range: 11.6,
            y_offset: 0.50,
        },
        124 => BlockLightSpec {
            color: Color::srgb(1.0, 0.95, 0.86),
            intensity: 1160.0,
            range: 12.0,
            y_offset: 0.50,
        },
        138 => BlockLightSpec {
            color: Color::srgb(0.72, 0.90, 1.0),
            intensity: 940.0,
            range: 11.0,
            y_offset: 0.50,
        },
        169 => BlockLightSpec {
            color: Color::srgb(0.72, 0.97, 0.95),
            intensity: 1160.0,
            range: 12.2,
            y_offset: 0.50,
        },
        _ => return None,
    };
    Some(spec)
}

fn chunk_block_state_at(store: &chunk::ChunkStore, pos: IVec3) -> u16 {
    if !(0..256).contains(&pos.y) {
        return 0;
    }
    let chunk_x = pos.x.div_euclid(16);
    let chunk_z = pos.z.div_euclid(16);
    let local_x = pos.x.rem_euclid(16) as usize;
    let local_z = pos.z.rem_euclid(16) as usize;
    let Some(column) = store.chunks.get(&(chunk_x, chunk_z)) else {
        return 0;
    };
    let section_index = (pos.y / 16) as usize;
    let local_y = (pos.y % 16) as usize;
    let Some(section) = column.sections.get(section_index).and_then(|v| v.as_ref()) else {
        return 0;
    };
    let idx = local_y * 16 * 16 + local_z * 16 + local_x;
    section.get(idx).copied().unwrap_or(0)
}

fn is_exposed_light_block(store: &chunk::ChunkStore, pos: IVec3) -> bool {
    for d in [
        IVec3::new(1, 0, 0),
        IVec3::new(-1, 0, 0),
        IVec3::new(0, 1, 0),
        IVec3::new(0, -1, 0),
        IVec3::new(0, 0, 1),
        IVec3::new(0, 0, -1),
    ] {
        if block_state_id(chunk_block_state_at(store, pos + d)) == 0 {
            return true;
        }
    }
    false
}

fn is_light_blocking_geometry(block_state: u16) -> bool {
    let id = block_state_id(block_state);
    if id == 0 {
        return false;
    }
    if matches!(id, 8 | 9 | 10 | 11) {
        return false;
    }
    !matches!(
        block_model_kind(id),
        BlockModelKind::Cross | BlockModelKind::TorchLike
    )
}

fn is_camera_to_light_occluded(
    store: &chunk::ChunkStore,
    camera_pos: Vec3,
    light_pos: Vec3,
) -> bool {
    let to_light = light_pos - camera_pos;
    let dist = to_light.length();
    if dist <= 0.05 {
        return false;
    }
    let dir = to_light / dist;
    let start = 0.35f32;
    let end = (dist - 0.30).max(start);
    let steps = ((end - start) / 0.20).ceil().max(1.0) as i32;
    for i in 0..=steps {
        let t = start + (end - start) * (i as f32 / steps as f32);
        let p = camera_pos + dir * t;
        let cell = p.floor().as_ivec3();
        if is_light_blocking_geometry(chunk_block_state_at(store, cell)) {
            return true;
        }
    }
    false
}

pub(super) fn update_dynamic_block_lights(
    mut commands: Commands,
    time: Res<Time>,
    store: Res<chunk::ChunkStore>,
    camera_query: Query<&GlobalTransform, With<components::PlayerCamera>>,
    mut state: ResMut<DynamicBlockLightState>,
) {
    state.refresh_timer.tick(time.delta());
    if !state.refresh_timer.just_finished() {
        return;
    }

    let Ok(camera) = camera_query.get_single() else {
        for (_, e) in state.by_pos.drain() {
            commands.entity(e).despawn_recursive();
        }
        return;
    };
    let cam_pos = camera.translation().floor().as_ivec3();

    let mut candidates = Vec::<(f32, IVec3, BlockLightSpec)>::new();
    for y in (cam_pos.y - DYNAMIC_LIGHT_SCAN_RADIUS_Y)..=(cam_pos.y + DYNAMIC_LIGHT_SCAN_RADIUS_Y) {
        if !(0..256).contains(&y) {
            continue;
        }
        for z in
            (cam_pos.z - DYNAMIC_LIGHT_SCAN_RADIUS_XZ)..=(cam_pos.z + DYNAMIC_LIGHT_SCAN_RADIUS_XZ)
        {
            for x in (cam_pos.x - DYNAMIC_LIGHT_SCAN_RADIUS_XZ)
                ..=(cam_pos.x + DYNAMIC_LIGHT_SCAN_RADIUS_XZ)
            {
                let pos = IVec3::new(x, y, z);
                let state_id = chunk_block_state_at(&store, pos);
                let Some(spec) = block_light_spec(state_id) else {
                    continue;
                };
                if !is_exposed_light_block(&store, pos) {
                    continue;
                }
                let world = Vec3::new(x as f32 + 0.5, y as f32 + spec.y_offset, z as f32 + 0.5);
                if is_camera_to_light_occluded(&store, camera.translation(), world) {
                    continue;
                }
                let d2 = camera.translation().distance_squared(world);
                candidates.push((d2, pos, spec));
            }
        }
    }
    candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    candidates.truncate(DYNAMIC_LIGHT_MAX_COUNT);

    let desired: HashMap<IVec3, BlockLightSpec> = candidates
        .into_iter()
        .map(|(_, pos, spec)| (pos, spec))
        .collect();

    let mut to_remove = Vec::new();
    for (pos, entity) in &state.by_pos {
        if !desired.contains_key(pos) {
            commands.entity(*entity).despawn_recursive();
            to_remove.push(*pos);
        }
    }
    for pos in to_remove {
        state.by_pos.remove(&pos);
    }

    for (pos, spec) in desired {
        let t = Transform::from_xyz(
            pos.x as f32 + 0.5,
            pos.y as f32 + spec.y_offset,
            pos.z as f32 + 0.5,
        );
        if let Some(entity) = state.by_pos.get(&pos).copied() {
            commands.entity(entity).insert((
                PointLight {
                    color: spec.color,
                    intensity: spec.intensity * DYNAMIC_LIGHT_INTENSITY_SCALE,
                    range: spec.range * DYNAMIC_LIGHT_RANGE_SCALE,
                    radius: 0.18,
                    shadows_enabled: false,
                    ..default()
                },
                t,
            ));
        } else {
            let entity = commands
                .spawn((
                    DynamicBlockLight,
                    PointLight {
                        color: spec.color,
                        intensity: spec.intensity * DYNAMIC_LIGHT_INTENSITY_SCALE,
                        range: spec.range * DYNAMIC_LIGHT_RANGE_SCALE,
                        radius: 0.18,
                        shadows_enabled: false,
                        ..default()
                    },
                    t,
                    GlobalTransform::default(),
                    RenderLayers::layer(reflection::MAIN_RENDER_LAYER)
                        .with(reflection::CHUNK_OPAQUE_RENDER_LAYER)
                        .with(reflection::CHUNK_CUTOUT_RENDER_LAYER)
                        .with(reflection::CHUNK_TRANSPARENT_RENDER_LAYER),
                ))
                .id();
            state.by_pos.insert(pos, entity);
        }
    }
}
