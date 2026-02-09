use bevy::prelude::*;

use std::collections::HashMap;

use crate::async_mesh::{MeshAsyncResources, MeshInFlight, MeshJob};
use crate::chunk::{snapshot_for_chunk, ChunkStore};
use crate::components::{ChunkRoot, Player, PlayerCamera, ShadowCasterLight};
use bevy::pbr::wireframe::WireframeConfig;
use bevy::prelude::{ChildOf, GlobalTransform, Mesh3d, PerspectiveProjection, Projection, Vec3};
use bevy::render::camera::CameraProjection;
use bevy::render::primitives::{Aabb, Frustum};
use bevy::render::view::ViewVisibility;

#[derive(Resource, Debug, Clone)]
pub struct RenderDebugSettings {
    pub shadows_enabled: bool,
    pub render_distance_chunks: i32,
    pub fov_deg: f32,
    pub use_greedy_meshing: bool,
    pub wireframe_enabled: bool,
    pub manual_frustum_cull: bool,
    pub frustum_fov_debug: bool,
    pub frustum_fov_deg: f32,
}

impl Default for RenderDebugSettings {
    fn default() -> Self {
        Self {
            shadows_enabled: true,
            render_distance_chunks: 12,
            fov_deg: 110.0,
            use_greedy_meshing: true,
            wireframe_enabled: false,
            manual_frustum_cull: true,
            frustum_fov_debug: false,
            frustum_fov_deg: 110.0,
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
    pub last_updates_raw: u32,
    pub total_meshes: u32,
    pub visible_meshes_distance: u32,
    pub visible_meshes_view: u32,
    pub total_chunks: u32,
    pub visible_chunks: u32,
    pub apply_debug_ms: f32,
    pub gather_stats_ms: f32,
}

pub fn apply_render_debug_settings(
    settings: Res<RenderDebugSettings>,
    mut lights: Query<(&mut DirectionalLight, Option<&ShadowCasterLight>)>,
    player: Query<&Transform, With<Player>>,
    mut params: ParamSet<(
        Query<(Entity, &ChunkRoot, &mut Visibility)>,
        Query<(&ChildOf, &mut Visibility), With<Mesh3d>>,
    )>,
    mut cameras: Query<&mut Projection, With<PlayerCamera>>,
    mut wireframe: ResMut<WireframeConfig>,
    mut perf: ResMut<RenderPerfStats>,
) {
    let start = std::time::Instant::now();
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

    let mut chunk_visibility = HashMap::new();
    for (entity, chunk, mut visibility) in &mut params.p0() {
        let dx = (chunk.key.0 - player_chunk_x).abs();
        let dz = (chunk.key.1 - player_chunk_z).abs();
        let visible = dx <= max_dist && dz <= max_dist;
        let vis = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        *visibility = vis;
        chunk_visibility.insert(entity, vis);
    }

    for (parent, mut visibility) in &mut params.p1() {
        if let Some(parent_vis) = chunk_visibility.get(&parent.parent()) {
            *visibility = match parent_vis {
                Visibility::Visible | Visibility::Inherited => Visibility::Inherited,
                Visibility::Hidden => Visibility::Hidden,
            };
        }
    }
    perf.apply_debug_ms = start.elapsed().as_secs_f32() * 1000.0;
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

pub fn gather_render_stats(
    mut perf: ResMut<RenderPerfStats>,
    meshes: Query<(&Visibility, &ViewVisibility), With<Mesh3d>>,
    chunks: Query<&Visibility, With<ChunkRoot>>,
) {
    let start = std::time::Instant::now();
    let mut total_meshes = 0u32;
    let mut visible_meshes_distance = 0u32;
    let mut visible_meshes_view = 0u32;
    for (vis, view_vis) in &meshes {
        total_meshes += 1;
        if !matches!(*vis, Visibility::Hidden) {
            visible_meshes_distance += 1;
        }
        if view_vis.get() {
            visible_meshes_view += 1;
        }
    }

    let mut total_chunks = 0u32;
    let mut visible_chunks = 0u32;
    for vis in &chunks {
        total_chunks += 1;
        if matches!(*vis, Visibility::Visible) {
            visible_chunks += 1;
        }
    }

    perf.total_meshes = total_meshes;
    perf.visible_meshes_distance = visible_meshes_distance;
    perf.visible_meshes_view = visible_meshes_view;
    perf.total_chunks = total_chunks;
    perf.visible_chunks = visible_chunks;
    perf.gather_stats_ms = start.elapsed().as_secs_f32() * 1000.0;
}

pub fn manual_frustum_cull(
    settings: Res<RenderDebugSettings>,
    camera_query: Query<(&GlobalTransform, &Projection), With<PlayerCamera>>,
    mut params: ParamSet<(
        Query<&Visibility, With<ChunkRoot>>,
        Query<(&ChildOf, &GlobalTransform, &Aabb, &mut Visibility), With<Mesh3d>>,
    )>,
) {
    if !settings.manual_frustum_cull {
        return;
    }
    let Ok((cam_transform, projection)) = camera_query.get_single() else {
        return;
    };
    let frustum = compute_camera_frustum(&settings, projection, cam_transform);

    for (parent, transform, aabb, mut visibility) in &mut params.p1() {
        if let Ok(parent_vis) = params.p0().get(parent.parent()) {
            if matches!(*parent_vis, Visibility::Hidden) {
                *visibility = Visibility::Hidden;
                continue;
            }
        }

        let visible = frustum.intersects_obb(aabb, &transform.affine(), true, true);
        *visibility = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn compute_camera_frustum(
    settings: &RenderDebugSettings,
    projection: &Projection,
    cam_transform: &GlobalTransform,
) -> Frustum {
    match projection {
        Projection::Perspective(persp) => {
            if settings.frustum_fov_debug {
                let mut custom = persp.clone();
                custom.fov = settings.frustum_fov_deg.max(1.0).to_radians();
                custom.compute_frustum(cam_transform)
            } else {
                persp.compute_frustum(cam_transform)
            }
        }
        _ => projection.compute_frustum(cam_transform),
    }
}
