use bevy::prelude::*;

use std::collections::HashMap;

use crate::async_mesh::{MeshAsyncResources, MeshInFlight, MeshJob};
use crate::chunk::{ChunkRenderAssets, ChunkStore, snapshot_for_chunk};
use crate::components::{ChunkRoot, Player, PlayerCamera, ShadowCasterLight};
use bevy::core_pipeline::fxaa::Fxaa;
use bevy::pbr::wireframe::WireframeConfig;
use bevy::prelude::{ChildOf, GlobalTransform, Mesh3d, Projection};
use bevy::render::primitives::Aabb;
use bevy::render::view::ViewVisibility;

const MANUAL_CULL_NEAR_DISABLE_DISTANCE: f32 = 8.0;

#[derive(Resource, Debug, Clone)]
pub struct RenderDebugSettings {
    pub shadows_enabled: bool,
    pub render_distance_chunks: i32,
    pub fov_deg: f32,
    pub use_greedy_meshing: bool,
    pub wireframe_enabled: bool,
    pub fxaa_enabled: bool,
    pub manual_frustum_cull: bool,
    pub frustum_fov_debug: bool,
    pub frustum_fov_deg: f32,
    pub show_chunk_borders: bool,
    pub show_coordinates: bool,
    pub show_look_info: bool,
    pub show_look_ray: bool,
    pub render_held_items: bool,
    pub render_first_person_arms: bool,
    pub render_self_model: bool,
}

impl Default for RenderDebugSettings {
    fn default() -> Self {
        Self {
            shadows_enabled: true,
            render_distance_chunks: 12,
            fov_deg: 110.0,
            use_greedy_meshing: true,
            wireframe_enabled: false,
            fxaa_enabled: true,
            manual_frustum_cull: true,
            frustum_fov_debug: false,
            frustum_fov_deg: 110.0,
            show_chunk_borders: false,
            show_coordinates: false,
            show_look_info: false,
            show_look_ray: false,
            render_held_items: true,
            render_first_person_arms: true,
            render_self_model: true,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct MeshingToggleState {
    pub last_use_greedy: bool,
}

impl Default for MeshingToggleState {
    fn default() -> Self {
        Self {
            last_use_greedy: true,
        }
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
    pub manual_cull_ms: f32,
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
    mut fxaa_query: Query<&mut Fxaa, With<PlayerCamera>>,
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
        for mut fxaa in &mut fxaa_query {
            fxaa.enabled = settings.fxaa_enabled;
        }
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
    assets: Res<ChunkRenderAssets>,
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
            texture_mapping: assets.texture_mapping.clone(),
            biome_tints: assets.biome_tints.clone(),
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
        Query<(Entity, &Visibility), With<ChunkRoot>>,
        Query<(&ChildOf, &GlobalTransform, &Aabb, &mut Visibility), With<Mesh3d>>,
    )>,
    mut perf: ResMut<RenderPerfStats>,
) {
    if !settings.manual_frustum_cull {
        perf.manual_cull_ms = 0.0;
        return;
    }
    let start = std::time::Instant::now();
    let Ok((cam_transform, projection)) = camera_query.get_single() else {
        return;
    };
    let (fov_y, aspect, near, far) = camera_fov_params(&settings, projection);
    let (forward, right, up, cam_pos) = (
        cam_transform.forward(),
        cam_transform.right(),
        cam_transform.up(),
        cam_transform.translation(),
    );
    let tan_y = (fov_y * 0.5).tan();
    let tan_x = tan_y * aspect;
    let chunk_visibility: HashMap<Entity, Visibility> = {
        let chunks = params.p0();
        let mut map = HashMap::new();
        for (entity, vis) in chunks.iter() {
            map.insert(entity, *vis);
        }
        map
    };

    for (parent, transform, aabb, mut visibility) in &mut params.p1() {
        if let Some(parent_vis) = chunk_visibility.get(&parent.parent()) {
            if matches!(parent_vis, Visibility::Hidden) {
                *visibility = Visibility::Hidden;
                continue;
            }
        }

        // Fast path: chunk sub-meshes are unscaled, so use translation directly.
        let center = transform.translation() + Vec3::from(aabb.center);
        let half = Vec3::from(aabb.half_extents);
        let cull_pad = 2.0;
        let radius = half.length() + cull_pad;
        let to_center = center - cam_pos;
        if to_center.length_squared()
            <= (MANUAL_CULL_NEAR_DISABLE_DISTANCE + radius)
                * (MANUAL_CULL_NEAR_DISABLE_DISTANCE + radius)
        {
            *visibility = Visibility::Inherited;
            continue;
        }
        let z = to_center.dot(*forward);
        if z < -radius - cull_pad {
            *visibility = Visibility::Hidden;
            continue;
        }
        let x = to_center.dot(*right).abs();
        let y = to_center.dot(*up).abs();
        let visible = x <= z * tan_x + radius
            && y <= z * tan_y + radius
            && z <= far + radius + cull_pad
            && z >= near - radius - cull_pad;
        *visibility = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    perf.manual_cull_ms = start.elapsed().as_secs_f32() * 1000.0;
}

fn camera_fov_params(
    settings: &RenderDebugSettings,
    projection: &Projection,
) -> (f32, f32, f32, f32) {
    let (mut fov_y, mut aspect, mut near, mut far) = match projection {
        Projection::Perspective(p) => (p.fov, p.aspect_ratio, p.near, p.far),
        _ => (settings.fov_deg.to_radians(), 1.0, 0.1, 1000.0),
    };
    // Keep culling stable even when the camera FOV is temporarily modified (e.g. zoom).
    fov_y = fov_y.max(settings.fov_deg.to_radians());
    if settings.frustum_fov_debug {
        fov_y = settings.frustum_fov_deg.max(1.0).to_radians();
    }
    // Expand FOV to reduce border clipping artifacts.
    fov_y = (fov_y * 1.40).min(std::f32::consts::PI - 0.01);
    aspect = aspect.max(0.01);
    near = near.max(0.01);
    far = far.max(near + 0.01);
    (fov_y, aspect, near, far)
}
