use bevy::prelude::*;

use crate::async_mesh::{MeshAsyncResources, MeshInFlight, MeshJob};
use crate::chunk::{snapshot_for_chunk, ChunkStore};
use crate::components::{ChunkRoot, Player, PlayerCamera, ShadowCasterLight};
use bevy::pbr::wireframe::WireframeConfig;

#[derive(Resource, Debug, Clone)]
pub struct RenderDebugSettings {
    pub shadows_enabled: bool,
    pub render_distance_chunks: i32,
    pub fov_deg: f32,
    pub use_greedy_meshing: bool,
    pub wireframe_enabled: bool,
}

impl Default for RenderDebugSettings {
    fn default() -> Self {
        Self {
            shadows_enabled: true,
            render_distance_chunks: 12,
            fov_deg: 110.0,
            use_greedy_meshing: true,
            wireframe_enabled: false,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct MeshingToggleState {
    pub last_use_greedy: bool,
}

impl Default for MeshingToggleState {
    fn default() -> Self {
        Self { last_use_greedy: true }
    }
}

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct RenderPerfStats {
    pub last_mesh_build_ms: f32,
    pub avg_mesh_build_ms: f32,
    pub last_apply_ms: f32,
    pub avg_apply_ms: f32,
    pub last_enqueue_ms: f32,
    pub avg_enqueue_ms: f32,
    pub last_meshes_applied: u32,
    pub in_flight: u32,
    pub last_updates: u32,
}

pub fn apply_render_debug_settings(
    settings: Res<RenderDebugSettings>,
    mut lights: Query<(&mut DirectionalLight, Option<&ShadowCasterLight>)>,
    player: Query<&Transform, With<Player>>,
    mut chunks: Query<(&ChunkRoot, &mut Visibility)>,
    mut cameras: Query<&mut Projection, With<PlayerCamera>>,
    mut wireframe: ResMut<WireframeConfig>,
) {
    if settings.is_changed() {
        for (mut light, is_shadow) in &mut lights {
            if is_shadow.is_some() {
                light.shadows_enabled = settings.shadows_enabled;
            }
        }
        for mut projection in &mut cameras {
            if let Projection::Perspective(persp) = &mut *projection {
                persp.fov = settings.fov_deg.to_radians();
            }
        }
        wireframe.global = settings.wireframe_enabled;
    }

    let Ok(player_transform) = player.get_single() else {
        return;
    };
    let player_chunk_x = (player_transform.translation.x / 16.0).floor() as i32;
    let player_chunk_z = (player_transform.translation.z / 16.0).floor() as i32;
    let max_dist = settings.render_distance_chunks.max(1);

    for (chunk, mut visibility) in &mut chunks {
        let dx = (chunk.key.0 - player_chunk_x).abs();
        let dz = (chunk.key.1 - player_chunk_z).abs();
        let visible = dx <= max_dist && dz <= max_dist;
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

pub fn remesh_on_meshing_toggle(
    settings: Res<RenderDebugSettings>,
    mut state: ResMut<MeshingToggleState>,
    store: Res<ChunkStore>,
    async_mesh: Res<MeshAsyncResources>,
    mut in_flight: ResMut<MeshInFlight>,
) {
    if settings.use_greedy_meshing == state.last_use_greedy {
        return;
    }
    state.last_use_greedy = settings.use_greedy_meshing;
    in_flight.chunks.clear();
    for key in store.chunks.keys().copied() {
        let snapshot = snapshot_for_chunk(&store, key);
        let job = MeshJob {
            chunk_key: key,
            snapshot,
            use_greedy: settings.use_greedy_meshing,
        };
        if async_mesh.job_tx.send(job).is_ok() {
            in_flight.chunks.insert(key);
        }
    }
}
