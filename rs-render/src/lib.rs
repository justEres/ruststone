use std::collections::HashMap;

use bevy::pbr::{MaterialPlugin, wireframe::WireframePlugin};
use bevy::prelude::*;
use bevy::render::view::VisibilitySystems;
use bevy::render::view::{InheritedVisibility, ViewVisibility, Visibility};

mod async_mesh;
mod block_textures;
mod camera;
mod chunk;
mod components;
pub mod debug;
mod input;
mod world;

pub use chunk::ChunkUpdateQueue;
pub use components::{
    ChunkRoot, LookAngles, Player, PlayerCamera, ShadowCasterLight, Velocity, WorldRoot,
};
pub use debug::RenderDebugSettings;

pub struct RenderPlugin;

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
        .add_systems(Startup, (world::setup_world, camera::spawn_player))
        .add_systems(
            Update,
            (
                input::apply_cursor_lock,
                debug::apply_render_debug_settings,
                debug::remesh_on_meshing_toggle,
                enqueue_chunk_meshes,
            ),
        )
        .add_systems(
            PostUpdate,
            (
                apply_mesh_results.before(VisibilitySystems::CheckVisibility),
                debug::manual_frustum_cull
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
) {
    let start = std::time::Instant::now();
    if queue.0.is_empty() {
        perf.in_flight = in_flight.chunks.len() as u32;
        return;
    }

    let mut updated_keys = std::collections::HashSet::new();
    let raw_updates = queue.0.len() as u32;
    for chunk in queue.0.drain(..) {
        let key = (chunk.x, chunk.z);
        chunk::update_store(&mut store, chunk);
        updated_keys.insert(key);
    }
    let updates_len = updated_keys.len() as u32;

    for key in updated_keys {
        if in_flight.chunks.contains(&key) {
            continue;
        }
        let snapshot = chunk::snapshot_for_chunk(&store, key);
        let job = async_mesh::MeshJob {
            chunk_key: key,
            snapshot,
            use_greedy: render_debug.use_greedy_meshing,
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

fn apply_mesh_results(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
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
            }
        });

        let mut active_keys = std::collections::HashSet::new();
        let chunk::MeshBatch {
            opaque,
            transparent,
        } = mesh_batch;
        for (group, data, material) in [
            (
                chunk::MaterialGroup::Opaque,
                opaque,
                assets.opaque_material.clone(),
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
            active_keys.insert(group);
            let (mesh, bounds) = chunk::build_mesh_from_data(data);

            if let Some(submesh) = entry.submeshes.get_mut(&group) {
                if let Some(existing) = meshes.get_mut(&submesh.mesh) {
                    *existing = mesh;
                } else {
                    let handle = meshes.add(mesh);
                    commands
                        .entity(submesh.entity)
                        .insert(Mesh3d(handle.clone()));
                    submesh.mesh = handle;
                }
                if let Some((min, max)) = bounds {
                    let center = (min + max) * 0.5;
                    let half = (max - min) * 0.5;
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
                        MeshMaterial3d(material),
                        Transform::default(),
                        GlobalTransform::default(),
                        Visibility::Inherited,
                        InheritedVisibility::default(),
                        ViewVisibility::default(),
                    ))
                    .id();
                if let Some((min, max)) = bounds {
                    let center = (min + max) * 0.5;
                    let half = (max - min) * 0.5;
                    commands
                        .entity(child)
                        .insert(bevy::render::primitives::Aabb {
                            center: center.into(),
                            half_extents: half.into(),
                        });
                }
                commands.entity(entry.entity).add_child(child);
                entry.submeshes.insert(
                    group,
                    chunk::SubmeshEntry {
                        entity: child,
                        mesh: handle,
                    },
                );
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
