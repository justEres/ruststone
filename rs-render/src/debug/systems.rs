use std::collections::HashMap;

use bevy::pbr::wireframe::WireframeConfig;
use bevy::prelude::{ChildOf, Mesh3d, Projection, *};
use bevy::render::view::ViewVisibility;

use crate::async_mesh::{MeshGeneration, MeshInFlight};
use crate::chunk::{ChunkRenderAssets, ChunkStore, PendingChunkRemesh};
use crate::components::{ChunkRoot, Player, PlayerCamera, ShadowCasterLight};

use super::{MeshingToggleState, RenderDebugSettings, RenderPerfStats, ShadingModel};

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
                persp.far = if settings.infinite_render_distance {
                    100_000.0
                } else {
                    (settings.render_distance_chunks.max(1) as f32 * 16.0 + 256.0).max(512.0)
                };
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
        let visible = settings.infinite_render_distance || (dx <= max_dist && dz <= max_dist);
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
    mut settings: ResMut<RenderDebugSettings>,
    mut state: ResMut<MeshingToggleState>,
    store: Res<ChunkStore>,
    mut pending: ResMut<PendingChunkRemesh>,
    mut generation: ResMut<MeshGeneration>,
    mut in_flight: ResMut<MeshInFlight>,
    _assets: Res<ChunkRenderAssets>,
) {
    if settings.use_greedy_meshing == state.last_use_greedy
        && settings.voxel_ao_enabled == state.last_voxel_ao_enabled
        && settings.voxel_ao_cutout == state.last_voxel_ao_cutout
        && (settings.voxel_ao_strength - state.last_voxel_ao_strength).abs() < 0.001
        && settings.barrier_billboard == state.last_barrier_billboard
        && settings.shading_model == state.last_shading_model
        && settings.vanilla_block_shadow_mode == state.last_vanilla_block_shadow_mode
        && (settings.vanilla_block_shadow_strength - state.last_vanilla_block_shadow_strength)
            .abs()
            < 0.001
        && settings.vanilla_sun_trace_samples == state.last_vanilla_sun_trace_samples
        && (settings.vanilla_sun_trace_distance - state.last_vanilla_sun_trace_distance).abs()
            < 0.001
        && (settings.vanilla_top_face_sun_bias - state.last_vanilla_top_face_sun_bias).abs()
            < 0.001
        && (settings.vanilla_face_shading_strength - state.last_vanilla_face_shading_strength)
            .abs()
            < 0.001
        && (settings.vanilla_ambient_floor - state.last_vanilla_ambient_floor).abs() < 0.001
        && (settings.vanilla_light_curve - state.last_vanilla_light_curve).abs() < 0.001
        && (settings.vanilla_foliage_tint_strength - state.last_vanilla_foliage_tint_strength)
            .abs()
            < 0.001
        && (settings.vanilla_sky_light_strength - state.last_vanilla_sky_light_strength).abs()
            < 0.001
        && (settings.vanilla_block_light_strength - state.last_vanilla_block_light_strength)
            .abs()
            < 0.001
        && (settings.vanilla_ao_shadow_blend - state.last_vanilla_ao_shadow_blend).abs()
            < 0.001
        && !settings.force_remesh
    {
        return;
    }
    state.last_use_greedy = settings.use_greedy_meshing;
    state.last_voxel_ao_enabled = settings.voxel_ao_enabled;
    state.last_voxel_ao_cutout = settings.voxel_ao_cutout;
    state.last_voxel_ao_strength = settings.voxel_ao_strength;
    state.last_barrier_billboard = settings.barrier_billboard;
    state.last_shading_model = settings.shading_model;
    state.last_vanilla_block_shadow_mode = settings.vanilla_block_shadow_mode;
    state.last_vanilla_block_shadow_strength = settings.vanilla_block_shadow_strength;
    state.last_vanilla_sun_trace_samples = settings.vanilla_sun_trace_samples;
    state.last_vanilla_sun_trace_distance = settings.vanilla_sun_trace_distance;
    state.last_vanilla_top_face_sun_bias = settings.vanilla_top_face_sun_bias;
    state.last_vanilla_face_shading_strength = settings.vanilla_face_shading_strength;
    state.last_vanilla_ambient_floor = settings.vanilla_ambient_floor;
    state.last_vanilla_light_curve = settings.vanilla_light_curve;
    state.last_vanilla_foliage_tint_strength = settings.vanilla_foliage_tint_strength;
    state.last_vanilla_sky_light_strength = settings.vanilla_sky_light_strength;
    state.last_vanilla_block_light_strength = settings.vanilla_block_light_strength;
    state.last_vanilla_ao_shadow_blend = settings.vanilla_ao_shadow_blend;
    settings.force_remesh = false;
    settings.clear_and_rebuild_meshes = false;
    generation.0 = generation.0.wrapping_add(1);
    in_flight.chunks.clear();
    in_flight.pending_remesh.clear();
    pending.keys.clear();
    pending.keys.extend(store.chunks.keys().copied());
}

pub fn refresh_render_state_on_mode_change(
    mut settings: ResMut<RenderDebugSettings>,
    mut last_mode: Local<Option<(u8, bool, ShadingModel)>>,
) {
    let mode = (
        settings.shader_quality_mode,
        settings.enable_pbr_terrain_lighting,
        settings.shading_model,
    );
    let changed = last_mode.map(|m| m != mode).unwrap_or(false);
    if changed {
        settings.material_rebuild_nonce = settings.material_rebuild_nonce.wrapping_add(1);
        settings.force_remesh = true;
    }
    *last_mode = Some(mode);
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
