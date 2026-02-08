use bevy::prelude::*;

mod async_mesh;
mod camera;
mod chunk;
mod components;
mod input;
mod movement;
mod world;

pub use chunk::ChunkUpdateQueue;
pub use components::{LookAngles, Player, PlayerCamera, Velocity, WorldRoot};

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<input::PlayerInput>()
            .init_resource::<movement::MovementSettings>()
            .init_resource::<world::WorldSettings>()
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
                    input::collect_player_input,
                    movement::apply_player_look,
                    movement::apply_player_movement,
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
        let mesh = chunk::build_mesh_from_data(result.mesh);

        if let Some(entry) = state.entries.get_mut(&key) {
            if let Some(existing) = meshes.get_mut(&entry.mesh) {
                *existing = mesh;
            } else {
                let handle = meshes.add(mesh);
                commands.entity(entry.entity).insert(Mesh3d(handle.clone()));
                entry.mesh = handle;
            }
        } else {
            let handle = meshes.add(mesh);
            let entity = commands
                .spawn((
                    Mesh3d(handle.clone()),
                    MeshMaterial3d(assets.material.clone()),
                    Transform::from_xyz((key.0 * 16) as f32, 0.0, (key.1 * 16) as f32),
                    GlobalTransform::default(),
                ))
                .id();

            state.entries.insert(key, chunk::ChunkEntry { entity, mesh: handle });
        }

        in_flight.chunks.remove(&key);
    }
}
