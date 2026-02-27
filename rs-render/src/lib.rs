use std::collections::HashMap;

use bevy::pbr::{MaterialPlugin, wireframe::WireframePlugin};
use bevy::prelude::*;
use bevy::prelude::Mesh3d;
use bevy::render::view::RenderLayers;
use bevy::render::view::VisibilitySystems;
use bevy::render::view::{InheritedVisibility, NoFrustumCulling, ViewVisibility, Visibility};
use rs_utils::block_state_id;

mod async_mesh;
mod block_models;
mod block_textures;
mod camera;
mod chunk;
mod components;
pub mod debug;
mod input;
mod lighting;
mod reflection;
mod world;

pub use block_models::{BlockModelResolver, IconQuad, default_model_roots};
pub use block_textures::{AtlasBlockMapping, Face as ModelFace, build_block_texture_mapping};
pub use chunk::{ChunkStore, ChunkUpdateQueue, WorldUpdate, apply_block_update};
pub use components::{
    ChunkRoot, LookAngles, Player, PlayerCamera, ShadowCasterLight, Velocity, WorldRoot,
};
pub use debug::{AntiAliasingMode, RenderDebugSettings};
pub use lighting::{LightingQualityPreset, ShadowQualityPreset};

pub struct RenderPlugin;

const VERTICAL_CULL_SECTION_HEIGHT: f32 = 16.0;
const DYNAMIC_LIGHT_SCAN_RADIUS_XZ: i32 = 20;
const DYNAMIC_LIGHT_SCAN_RADIUS_Y: i32 = 12;
const DYNAMIC_LIGHT_MAX_COUNT: usize = 96;
const DYNAMIC_LIGHT_REFRESH_SECONDS: f32 = 0.20;

#[derive(Component)]
struct DynamicBlockLight;

#[derive(Resource)]
struct DynamicBlockLightState {
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

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            WireframePlugin::default(),
            MaterialPlugin::<chunk::ChunkAtlasMaterial>::default(),
        ))
        .init_resource::<world::WorldSettings>()
        .init_resource::<debug::RenderDebugSettings>()
        .init_resource::<debug::MeshingToggleState>()
        .init_resource::<debug::RenderPerfStats>()
        .init_resource::<chunk::ChunkUpdateQueue>()
        .init_resource::<chunk::ChunkRenderState>()
        .init_resource::<chunk::ChunkStore>()
        .init_resource::<chunk::ChunkRenderAssets>()
        .init_resource::<async_mesh::MeshAsyncResources>()
        .init_resource::<async_mesh::MeshInFlight>()
        .init_resource::<DynamicBlockLightState>()
        .add_systems(Startup, (world::setup_world, camera::spawn_player))
        .add_systems(
            Update,
            (
                input::apply_cursor_lock,
                debug::apply_render_debug_settings,
                lighting::apply_lighting_quality.after(debug::apply_render_debug_settings),
                lighting::update_water_animation.after(lighting::apply_lighting_quality),
                lighting::update_material_debug_stats.after(lighting::update_water_animation),
                lighting::apply_antialiasing.after(debug::apply_render_debug_settings),
                lighting::apply_ssao_quality.after(lighting::apply_lighting_quality),
                lighting::apply_depth_prepass_for_ssr.after(lighting::apply_lighting_quality),
                debug::refresh_render_state_on_mode_change
                    .after(debug::apply_render_debug_settings),
                debug::remesh_on_meshing_toggle,
                enqueue_chunk_meshes,
                disable_engine_frustum_culling_globally,
                update_dynamic_block_lights,
            ),
        )
        .add_systems(
            PostUpdate,
            (
                apply_mesh_results.before(VisibilitySystems::CheckVisibility),
                debug::occlusion_cull_chunks
                    .after(apply_mesh_results)
                    .before(VisibilitySystems::CheckVisibility),
                debug::gather_render_stats.after(VisibilitySystems::CheckVisibility),
            ),
        );
    }
}

fn enqueue_chunk_meshes(
    mut queue: ResMut<chunk::ChunkUpdateQueue>,
    mut store: ResMut<chunk::ChunkStore>,
    async_mesh: Res<async_mesh::MeshAsyncResources>,
    mut in_flight: ResMut<async_mesh::MeshInFlight>,
    render_debug: Res<debug::RenderDebugSettings>,
    mut perf: ResMut<debug::RenderPerfStats>,
    assets: Res<chunk::ChunkRenderAssets>,
    camera_query: Query<&GlobalTransform, With<components::PlayerCamera>>,
) {
    let start = std::time::Instant::now();
    if queue.0.is_empty() {
        perf.in_flight = in_flight.chunks.len() as u32;
        return;
    }

    let mut updated_keys = std::collections::HashSet::new();
    let raw_updates = queue.0.len() as u32;
    for update in queue.0.drain(..) {
        match update {
            chunk::WorldUpdate::ChunkData(chunk) => {
                let key = (chunk.x, chunk.z);
                chunk::update_store(&mut store, chunk);
                updated_keys.insert(key);
                // If the neighbor chunk wasn't loaded when this chunk was meshed, it may have
                // generated border faces. Remesh neighbors when new chunk data arrives to
                // avoid chunk seam artifacts (notably visible with water).
                for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let nk = (key.0 + dx, key.1 + dz);
                    if store.chunks.contains_key(&nk) {
                        updated_keys.insert(nk);
                    }
                }
            }
            chunk::WorldUpdate::BlockUpdate(block_update) => {
                for key in chunk::apply_block_update(&mut store, block_update) {
                    updated_keys.insert(key);
                }
            }
        }
    }
    let updates_len = updated_keys.len() as u32;

    let mut ordered_keys = updated_keys.into_iter().collect::<Vec<_>>();
    if let Ok(cam) = camera_query.get_single() {
        let cam_pos = cam.translation();
        let cam_fwd = cam.forward();
        ordered_keys.sort_by(|a, b| {
            let sa = mesh_priority_score(*a, cam_pos, *cam_fwd);
            let sb = mesh_priority_score(*b, cam_pos, *cam_fwd);
            sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
        });
    } else {
        ordered_keys.sort_unstable();
    }

    for key in ordered_keys {
        if in_flight.chunks.contains(&key) {
            in_flight.pending_remesh.insert(key);
            continue;
        }
        let snapshot = chunk::snapshot_for_chunk(&store, key);
        let job = async_mesh::MeshJob {
            chunk_key: key,
            snapshot,
            use_greedy: render_debug.use_greedy_meshing,
            leaf_depth_layer_faces: true,
            voxel_ao_enabled: render_debug.voxel_ao_enabled,
            voxel_ao_strength: render_debug.voxel_ao_strength,
            voxel_ao_cutout: render_debug.voxel_ao_cutout,
            barrier_billboard: render_debug.barrier_billboard,
            texture_mapping: assets.texture_mapping.clone(),
            biome_tints: assets.biome_tints.clone(),
        };
        if async_mesh.job_tx.send(job).is_ok() {
            in_flight.chunks.insert(key);
        }
    }

    let elapsed_ms = start.elapsed().as_secs_f32() * 1000.0;
    perf.last_enqueue_ms = elapsed_ms;
    perf.avg_enqueue_ms = if perf.avg_enqueue_ms == 0.0 {
        elapsed_ms
    } else {
        perf.avg_enqueue_ms * 0.9 + elapsed_ms * 0.1
    };
    perf.last_updates = updates_len;
    perf.last_updates_raw = raw_updates;
    perf.in_flight = in_flight.chunks.len() as u32;
}

fn mesh_priority_score(key: (i32, i32), cam_pos: Vec3, cam_forward: Vec3) -> f32 {
    let center = Vec3::new((key.0 * 16 + 8) as f32, cam_pos.y, (key.1 * 16 + 8) as f32);
    let to = center - cam_pos;
    let dist2 = to.length_squared();
    let front_bias = to.normalize_or_zero().dot(cam_forward).max(0.0);
    dist2 - front_bias * 1024.0
}

fn apply_mesh_results(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    store: Res<chunk::ChunkStore>,
    render_debug: Res<debug::RenderDebugSettings>,
    assets: Res<chunk::ChunkRenderAssets>,
    mut state: ResMut<chunk::ChunkRenderState>,
    async_mesh: Res<async_mesh::MeshAsyncResources>,
    mut in_flight: ResMut<async_mesh::MeshInFlight>,
    mut perf: ResMut<debug::RenderPerfStats>,
) {
    let start = std::time::Instant::now();
    let mut applied = 0u32;
    let mut last_build_ms = 0.0f32;
    let mut receiver = async_mesh
        .result_rx
        .lock()
        .expect("mesh result receiver lock poisoned");

    while let Ok(result) = receiver.try_recv() {
        let key = result.chunk_key;
        let mesh_batch = result.mesh;
        last_build_ms = result.build_ms;

        let entry = state.entries.entry(key).or_insert_with(|| {
            let entity = commands
                .spawn((
                    Transform::from_xyz((key.0 * 16) as f32, 0.0, (key.1 * 16) as f32),
                    GlobalTransform::default(),
                    Visibility::Visible,
                    InheritedVisibility::default(),
                    ViewVisibility::default(),
                    ChunkRoot { key },
                ))
                .id();
            chunk::ChunkEntry {
                entity,
                submeshes: HashMap::new(),
                occlusion: chunk::ChunkOcclusionData::default(),
            }
        });

        let mut active_keys = std::collections::HashSet::new();
        let chunk::MeshBatch {
            opaque,
            cutout,
            cutout_culled,
            transparent,
            occlusion,
        } = mesh_batch;
        entry.occlusion = occlusion;
        for (group, data, material) in [
            (
                chunk::MaterialGroup::Opaque,
                opaque,
                assets.opaque_material.clone(),
            ),
            (
                chunk::MaterialGroup::Cutout,
                cutout,
                assets.cutout_material.clone(),
            ),
            (
                chunk::MaterialGroup::CutoutCulled,
                cutout_culled,
                assets.cutout_culled_material.clone(),
            ),
            (
                chunk::MaterialGroup::Transparent,
                transparent,
                assets.transparent_material.clone(),
            ),
        ] {
            if data.positions.is_empty() {
                continue;
            }
            let mesh_layers = match group {
                chunk::MaterialGroup::Opaque => {
                    RenderLayers::layer(reflection::CHUNK_OPAQUE_RENDER_LAYER)
                }
                chunk::MaterialGroup::Cutout | chunk::MaterialGroup::CutoutCulled => {
                    RenderLayers::layer(reflection::CHUNK_CUTOUT_RENDER_LAYER)
                }
                chunk::MaterialGroup::Transparent => {
                    RenderLayers::layer(reflection::CHUNK_TRANSPARENT_RENDER_LAYER)
                }
            };
            for (section, section_data) in split_mesh_data_vertical_sections(data) {
                let submesh_key = chunk::SubmeshKey { group, section };
                active_keys.insert(submesh_key);
                let (mesh, bounds) = chunk::build_mesh_from_data(section_data);

                if let Some(submesh) = entry.submeshes.get_mut(&submesh_key) {
                    if let Some(existing) = meshes.get_mut(&submesh.mesh) {
                        *existing = mesh;
                    } else {
                        let handle = meshes.add(mesh);
                        commands
                            .entity(submesh.entity)
                            .insert((Mesh3d(handle.clone()), mesh_layers.clone()));
                        submesh.mesh = handle;
                    }
                    commands.entity(submesh.entity).insert(mesh_layers.clone());
                    if let Some((min, max)) = bounds {
                        let center = (min + max) * 0.5;
                        let half = (max - min) * 0.5 + Vec3::splat(0.75);
                        commands
                            .entity(submesh.entity)
                            .insert(bevy::render::primitives::Aabb {
                                center: center.into(),
                                half_extents: half.into(),
                            });
                    }
                } else {
                    let handle = meshes.add(mesh);
                    let child = commands
                        .spawn((
                            Mesh3d(handle.clone()),
                            MeshMaterial3d(material.clone()),
                            mesh_layers.clone(),
                            Transform::default(),
                            GlobalTransform::default(),
                            Visibility::Inherited,
                            InheritedVisibility::default(),
                            ViewVisibility::default(),
                        ))
                        .id();
                    if let Some((min, max)) = bounds {
                        let center = (min + max) * 0.5;
                        let half = (max - min) * 0.5 + Vec3::splat(0.75);
                        commands
                            .entity(child)
                            .insert(bevy::render::primitives::Aabb {
                                center: center.into(),
                                half_extents: half.into(),
                            });
                    }
                    commands.entity(entry.entity).add_child(child);
                    entry.submeshes.insert(
                        submesh_key,
                        chunk::SubmeshEntry {
                            entity: child,
                            mesh: handle,
                        },
                    );
                }
            }
        }

        let mut remove_keys = Vec::new();
        for (key, submesh) in entry.submeshes.iter() {
            if !active_keys.contains(key) {
                commands.entity(submesh.entity).despawn_recursive();
                remove_keys.push(*key);
            }
        }
        for key in remove_keys {
            entry.submeshes.remove(&key);
        }

        in_flight.chunks.remove(&key);
        if in_flight.pending_remesh.remove(&key) {
            let snapshot = chunk::snapshot_for_chunk(&store, key);
            let job = async_mesh::MeshJob {
                chunk_key: key,
                snapshot,
                use_greedy: render_debug.use_greedy_meshing,
                leaf_depth_layer_faces: true,
                voxel_ao_enabled: render_debug.voxel_ao_enabled,
                voxel_ao_strength: render_debug.voxel_ao_strength,
                voxel_ao_cutout: render_debug.voxel_ao_cutout,
                barrier_billboard: render_debug.barrier_billboard,
                texture_mapping: assets.texture_mapping.clone(),
                biome_tints: assets.biome_tints.clone(),
            };
            if async_mesh.job_tx.send(job).is_ok() {
                in_flight.chunks.insert(key);
            }
        }
        applied += 1;
    }

    let elapsed_ms = start.elapsed().as_secs_f32() * 1000.0;
    perf.last_apply_ms = elapsed_ms;
    perf.avg_apply_ms = if perf.avg_apply_ms == 0.0 {
        elapsed_ms
    } else {
        perf.avg_apply_ms * 0.9 + elapsed_ms * 0.1
    };
    perf.last_mesh_build_ms = last_build_ms;
    perf.avg_mesh_build_ms = if perf.avg_mesh_build_ms == 0.0 {
        last_build_ms
    } else {
        perf.avg_mesh_build_ms * 0.9 + last_build_ms * 0.1
    };
    perf.last_meshes_applied = applied;
    perf.in_flight = in_flight.chunks.len() as u32;
}

fn split_mesh_data_vertical_sections(data: chunk::MeshData) -> Vec<(u8, chunk::MeshData)> {
    use std::collections::HashMap;

    let mut by_section: HashMap<u8, chunk::MeshData> = HashMap::new();
    for tri in data.indices.chunks_exact(3) {
        let i0 = tri[0] as usize;
        let i1 = tri[1] as usize;
        let i2 = tri[2] as usize;
        let y0 = data.positions[i0][1];
        let y1 = data.positions[i1][1];
        let y2 = data.positions[i2][1];
        let center_y = (y0 + y1 + y2) / 3.0;
        let section = ((center_y / VERTICAL_CULL_SECTION_HEIGHT).floor() as i32).clamp(0, 15) as u8;
        let section_mesh = by_section
            .entry(section)
            .or_insert_with(chunk::MeshData::empty);

        for &src in &[i0, i1, i2] {
            section_mesh.push_pos(data.positions[src]);
            section_mesh.normals.push(data.normals[src]);
            section_mesh.uvs.push(data.uvs[src]);
            section_mesh.uvs_b.push(data.uvs_b[src]);
            section_mesh.colors.push(data.colors[src]);
        }
        let base = section_mesh.positions.len() as u32 - 3;
        section_mesh
            .indices
            .extend_from_slice(&[base, base + 1, base + 2]);
    }

    let mut out = by_section.into_iter().collect::<Vec<_>>();
    out.sort_by_key(|(section, _)| *section);
    out
}

fn disable_engine_frustum_culling_globally(
    mut commands: Commands,
    meshes: Query<Entity, (With<Mesh3d>, Without<NoFrustumCulling>)>,
) {
    for entity in &meshes {
        commands.entity(entity).insert(NoFrustumCulling);
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
        }, // torch
        76 => BlockLightSpec {
            color: Color::srgb(1.0, 0.20, 0.18),
            intensity: 230.0,
            range: 6.2,
            y_offset: 0.62,
        }, // redstone torch on
        51 => BlockLightSpec {
            color: Color::srgb(1.0, 0.57, 0.24),
            intensity: 860.0,
            range: 10.5,
            y_offset: 0.55,
        }, // fire
        10 | 11 => BlockLightSpec {
            color: Color::srgb(1.0, 0.42, 0.12),
            intensity: 980.0,
            range: 11.0,
            y_offset: 0.50,
        }, // lava
        62 => BlockLightSpec {
            color: Color::srgb(1.0, 0.62, 0.33),
            intensity: 450.0,
            range: 7.8,
            y_offset: 0.50,
        }, // lit furnace
        74 => BlockLightSpec {
            color: Color::srgb(1.0, 0.22, 0.20),
            intensity: 250.0,
            range: 6.4,
            y_offset: 0.50,
        }, // glowing redstone ore
        89 => BlockLightSpec {
            color: Color::srgb(1.0, 0.94, 0.74),
            intensity: 1200.0,
            range: 12.8,
            y_offset: 0.50,
        }, // glowstone
        90 => BlockLightSpec {
            color: Color::srgb(0.72, 0.34, 1.0),
            intensity: 520.0,
            range: 8.2,
            y_offset: 0.50,
        }, // nether portal
        91 => BlockLightSpec {
            color: Color::srgb(1.0, 0.88, 0.56),
            intensity: 1020.0,
            range: 11.6,
            y_offset: 0.50,
        }, // jack o' lantern
        124 => BlockLightSpec {
            color: Color::srgb(1.0, 0.95, 0.86),
            intensity: 1160.0,
            range: 12.0,
            y_offset: 0.50,
        }, // lit redstone lamp
        138 => BlockLightSpec {
            color: Color::srgb(0.72, 0.90, 1.0),
            intensity: 940.0,
            range: 11.0,
            y_offset: 0.50,
        }, // beacon
        169 => BlockLightSpec {
            color: Color::srgb(0.72, 0.97, 0.95),
            intensity: 1160.0,
            range: 12.2,
            y_offset: 0.50,
        }, // sea lantern
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

fn update_dynamic_block_lights(
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
            for x in
                (cam_pos.x - DYNAMIC_LIGHT_SCAN_RADIUS_XZ)..=(cam_pos.x + DYNAMIC_LIGHT_SCAN_RADIUS_XZ)
            {
                let pos = IVec3::new(x, y, z);
                let state_id = chunk_block_state_at(&store, pos);
                let Some(spec) = block_light_spec(state_id) else {
                    continue;
                };
                if !is_exposed_light_block(&store, pos) {
                    continue;
                }
                let world =
                    Vec3::new(x as f32 + 0.5, y as f32 + spec.y_offset, z as f32 + 0.5);
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
                    intensity: spec.intensity,
                    range: spec.range,
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
                        intensity: spec.intensity,
                        range: spec.range,
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
