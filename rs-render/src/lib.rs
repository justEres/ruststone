use std::collections::HashMap;

use bevy::prelude::*;

mod async_mesh;
mod block_textures;
mod camera;
mod chunk;
mod components;
mod input;
mod world;

pub use chunk::ChunkUpdateQueue;
pub use components::{LookAngles, Player, PlayerCamera, Velocity, WorldRoot};

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<world::WorldSettings>()
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
                    enqueue_chunk_meshes,
                ),
            )
            .add_systems(PostUpdate, apply_mesh_results);
    }
}

fn enqueue_chunk_meshes(
    mut queue: ResMut<chunk::ChunkUpdateQueue>,
    mut store: ResMut<chunk::ChunkStore>,
    async_mesh: Res<async_mesh::MeshAsyncResources>,
    mut in_flight: ResMut<async_mesh::MeshInFlight>,
) {
    if queue.0.is_empty() {
        return;
    }

    let mut updated_keys = Vec::new();
    for chunk in queue.0.drain(..) {
        let key = (chunk.x, chunk.z);
        chunk::update_store(&mut store, chunk);
        updated_keys.push(key);
    }

    for key in updated_keys {
        if in_flight.chunks.contains(&key) {
            continue;
        }
        let snapshot = chunk::snapshot_for_chunk(&store, key);
        let job = async_mesh::MeshJob { chunk_key: key, snapshot };
        if async_mesh.job_tx.send(job).is_ok() {
            in_flight.chunks.insert(key);
        }
    }
}

fn apply_mesh_results(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    assets: Res<chunk::ChunkRenderAssets>,
    mut state: ResMut<chunk::ChunkRenderState>,
    async_mesh: Res<async_mesh::MeshAsyncResources>,
    mut in_flight: ResMut<async_mesh::MeshInFlight>,
) {
    let mut receiver = async_mesh
        .result_rx
        .lock()
        .expect("mesh result receiver lock poisoned");

    while let Ok(result) = receiver.try_recv() {
        let key = result.chunk_key;
        let mesh_batch = result.mesh;

        let entry = state.entries.entry(key).or_insert_with(|| {
            let entity = commands
                .spawn(SpatialBundle::from_transform(Transform::from_xyz(
                    (key.0 * 16) as f32,
                    0.0,
                    (key.1 * 16) as f32,
                )))
                .id();
            chunk::ChunkEntry {
                entity,
                submeshes: HashMap::new(),
            }
        });

        let mut active_keys = std::collections::HashSet::new();

        for (texture_key, data) in mesh_batch.meshes {
            active_keys.insert(texture_key);
            let mesh = chunk::build_mesh_from_data(data);

            if let Some(submesh) = entry.submeshes.get_mut(&texture_key) {
                if let Some(existing) = meshes.get_mut(&submesh.mesh) {
                    *existing = mesh;
                } else {
                    let handle = meshes.add(mesh);
                    commands.entity(submesh.entity).insert(Mesh3d(handle.clone()));
                    submesh.mesh = handle;
                }
            } else {
                let handle = meshes.add(mesh);
                let material = assets
                    .materials
                    .get(&texture_key)
                    .expect("missing material for texture key")
                    .clone();
                let child = commands
                    .spawn((
                        Mesh3d(handle.clone()),
                        MeshMaterial3d(material),
                        SpatialBundle::default(),
                    ))
                    .id();
                commands.entity(entry.entity).add_child(child);
                entry.submeshes.insert(
                    texture_key,
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
    }
}
