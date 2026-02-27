use bevy::prelude::*;

use std::collections::{HashMap, HashSet, VecDeque};

use crate::async_mesh::{MeshAsyncResources, MeshInFlight, MeshJob};
use crate::chunk::{
    ChunkFace, ChunkOcclusionData, ChunkRenderAssets, ChunkRenderState, ChunkStore,
    snapshot_for_chunk,
};
use crate::components::{ChunkRoot, Player, PlayerCamera, ShadowCasterLight};
use bevy::pbr::wireframe::WireframeConfig;
use bevy::prelude::{ChildOf, Mesh3d, Projection};
use bevy::render::primitives::Aabb;
use bevy::render::view::ViewVisibility;

use crate::lighting::{LightingQualityPreset, ShadowQualityPreset};

const MANUAL_CULL_NEAR_DISABLE_DISTANCE: f32 = 8.0;
const MANUAL_CULL_HORIZONTAL_FOV_MULTIPLIER: f32 = 1.30;
const MANUAL_CULL_PAD_BASE: f32 = 6.0;
const MANUAL_CULL_PAD_SHADOW_EXTRA: f32 = 12.0;
const OCCLUSION_CULL_HORIZONTAL_FOV_MULTIPLIER: f32 = 1.85;
const OCCLUSION_CULL_VERTICAL_FOV_MULTIPLIER: f32 = 1.60;
const OCCLUSION_CULL_RADIUS: f32 = 20.0;
const OCCLUSION_CULL_FRUSTUM_PAD: f32 = 24.0;
const OCCLUSION_CULL_Y_SAMPLES: [f32; 5] = [8.0, 64.0, 128.0, 192.0, 248.0];

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum AntiAliasingMode {
    Off,
    Fxaa,
    SmaaHigh,
    SmaaUltra,
    Msaa4,
    Msaa8,
}

impl Default for AntiAliasingMode {
    fn default() -> Self {
        Self::SmaaUltra
    }
}

impl AntiAliasingMode {
    pub const ALL: [Self; 6] = [
        Self::Off,
        Self::Fxaa,
        Self::SmaaHigh,
        Self::SmaaUltra,
        Self::Msaa4,
        Self::Msaa8,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Fxaa => "FXAA",
            Self::SmaaHigh => "SMAA High",
            Self::SmaaUltra => "SMAA Ultra",
            Self::Msaa4 => "MSAA 4x",
            Self::Msaa8 => "MSAA 8x",
        }
    }

    pub const fn as_options_value(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Fxaa => "fxaa",
            Self::SmaaHigh => "smaa_high",
            Self::SmaaUltra => "smaa_ultra",
            Self::Msaa4 => "msaa4",
            Self::Msaa8 => "msaa8",
        }
    }

    pub fn from_options_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "off" => Some(Self::Off),
            "fxaa" => Some(Self::Fxaa),
            "smaa_high" | "smaahigh" => Some(Self::SmaaHigh),
            "smaa_ultra" | "smaaultra" => Some(Self::SmaaUltra),
            "msaa4" | "msaa_4" => Some(Self::Msaa4),
            "msaa8" | "msaa_8" => Some(Self::Msaa8),
            _ => None,
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct RenderDebugSettings {
    pub shadows_enabled: bool,
    pub shadow_distance_scale: f32,
    pub render_distance_chunks: i32,
    pub fov_deg: f32,
    pub use_greedy_meshing: bool,
    pub wireframe_enabled: bool,
    pub aa_mode: AntiAliasingMode,
    pub fxaa_enabled: bool,
    pub manual_frustum_cull: bool,
    pub occlusion_cull_enabled: bool,
    pub frustum_fov_debug: bool,
    pub frustum_fov_deg: f32,
    pub show_chunk_borders: bool,
    pub show_coordinates: bool,
    pub show_look_info: bool,
    pub show_look_ray: bool,
    pub show_target_block_outline: bool,
    pub render_held_items: bool,
    pub render_first_person_arms: bool,
    pub render_self_model: bool,
    pub lighting_quality: LightingQualityPreset,
    pub shadow_quality: ShadowQualityPreset,
    pub shader_quality_mode: u8, // 0 fast .. 3 fancy
    pub enable_pbr_terrain_lighting: bool,
    pub sun_azimuth_deg: f32,
    pub sun_elevation_deg: f32,
    pub sun_strength: f32,
    pub ambient_strength: f32,
    pub ambient_brightness: f32,
    pub sun_illuminance: f32,
    pub fill_illuminance: f32,
    pub fog_density: f32,
    pub fog_start: f32,
    pub fog_end: f32,
    pub water_absorption: f32,
    pub water_fresnel: f32,
    pub shadow_map_size: u32,
    pub shadow_cascades: u8,
    pub shadow_max_distance: f32,
    pub shadow_first_cascade_far_bound: f32,
    pub shadow_depth_bias: f32,
    pub shadow_normal_bias: f32,
    pub color_saturation: f32,
    pub color_contrast: f32,
    pub color_brightness: f32,
    pub color_gamma: f32,
    pub voxel_ao_enabled: bool,
    pub voxel_ao_strength: f32,
    pub voxel_ao_cutout: bool,
    pub barrier_billboard: bool,
    pub water_reflections_enabled: bool,
    pub water_reflection_strength: f32,
    pub water_reflection_near_boost: f32,
    pub water_reflection_blue_tint: bool,
    pub water_reflection_tint_strength: f32,
    pub water_wave_strength: f32,
    pub water_wave_speed: f32,
    pub water_wave_detail_strength: f32,
    pub water_wave_detail_scale: f32,
    pub water_wave_detail_speed: f32,
    pub water_reflection_edge_fade: f32,
    pub water_reflection_overscan: f32,
    pub water_reflection_sky_fill: f32,
    pub water_reflection_screen_space: bool,
    pub water_ssr_steps: u8,
    pub water_ssr_thickness: f32,
    pub water_ssr_max_distance: f32,
    pub water_ssr_stride: f32,
    // Shader debug output mode:
    // 0 off, 1 pass id, 2 atlas rgb, 3 atlas alpha, 4 vertex tint, 5 linear depth
    pub cutout_debug_mode: u8,
    pub show_layer_entities: bool,
    pub show_layer_chunks_opaque: bool,
    pub show_layer_chunks_cutout: bool,
    pub show_layer_chunks_transparent: bool,
    pub force_remesh: bool,
    pub material_rebuild_nonce: u32,
}

impl Default for RenderDebugSettings {
    fn default() -> Self {
        Self {
            shadows_enabled: true,
            shadow_distance_scale: 1.0,
            render_distance_chunks: 12,
            fov_deg: 110.0,
            use_greedy_meshing: true,
            wireframe_enabled: false,
            aa_mode: AntiAliasingMode::default(),
            fxaa_enabled: true,
            manual_frustum_cull: true,
            occlusion_cull_enabled: true,
            frustum_fov_debug: false,
            frustum_fov_deg: 110.0,
            show_chunk_borders: false,
            show_coordinates: false,
            show_look_info: false,
            show_look_ray: false,
            show_target_block_outline: true,
            render_held_items: true,
            render_first_person_arms: true,
            render_self_model: true,
            lighting_quality: LightingQualityPreset::Standard,
            shadow_quality: ShadowQualityPreset::Medium,
            shader_quality_mode: 2,
            enable_pbr_terrain_lighting: false,
            sun_azimuth_deg: 62.0,
            sun_elevation_deg: 62.0,
            sun_strength: 0.56,
            ambient_strength: 0.52,
            ambient_brightness: 0.80,
            sun_illuminance: 11_500.0,
            fill_illuminance: 2_200.0,
            fog_density: 0.012,
            fog_start: 70.0,
            fog_end: 220.0,
            water_absorption: 0.18,
            water_fresnel: 0.12,
            shadow_map_size: 1536,
            shadow_cascades: 2,
            shadow_max_distance: 96.0,
            shadow_first_cascade_far_bound: 28.0,
            shadow_depth_bias: 0.022,
            shadow_normal_bias: 0.55,
            color_saturation: 1.08,
            color_contrast: 1.06,
            color_brightness: 0.0,
            color_gamma: 1.0,
            voxel_ao_enabled: true,
            voxel_ao_strength: 1.0,
            voxel_ao_cutout: true,
            barrier_billboard: true,
            water_reflections_enabled: true,
            water_reflection_strength: 0.85,
            water_reflection_near_boost: 0.18,
            water_reflection_blue_tint: false,
            water_reflection_tint_strength: 0.20,
            water_wave_strength: 0.42,
            water_wave_speed: 1.0,
            water_wave_detail_strength: 0.22,
            water_wave_detail_scale: 2.4,
            water_wave_detail_speed: 1.7,
            water_reflection_edge_fade: 0.22,
            water_reflection_overscan: 1.30,
            water_reflection_sky_fill: 0.55,
            water_reflection_screen_space: false,
            water_ssr_steps: 28,
            water_ssr_thickness: 0.24,
            water_ssr_max_distance: 80.0,
            water_ssr_stride: 1.25,
            cutout_debug_mode: 0,
            show_layer_entities: true,
            show_layer_chunks_opaque: true,
            show_layer_chunks_cutout: true,
            show_layer_chunks_transparent: true,
            force_remesh: false,
            material_rebuild_nonce: 0,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct MeshingToggleState {
    pub last_use_greedy: bool,
    pub last_voxel_ao_enabled: bool,
    pub last_voxel_ao_cutout: bool,
    pub last_voxel_ao_strength: f32,
    pub last_barrier_billboard: bool,
}

impl Default for MeshingToggleState {
    fn default() -> Self {
        Self {
            last_use_greedy: true,
            last_voxel_ao_enabled: true,
            last_voxel_ao_cutout: true,
            last_voxel_ao_strength: 1.0,
            last_barrier_billboard: true,
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
    pub occlusion_cull_ms: f32,
    pub visible_chunks_after_occlusion: u32,
    pub occluded_chunks: u32,
    pub mat_pass_opaque: f32,
    pub mat_pass_cutout: f32,
    pub mat_pass_cutout_culled: f32,
    pub mat_pass_transparent: f32,
    pub mat_alpha_opaque: u8,
    pub mat_alpha_cutout: u8,
    pub mat_alpha_cutout_culled: u8,
    pub mat_alpha_transparent: u8,
    pub mat_unlit_opaque: bool,
    pub mat_unlit_cutout: bool,
    pub mat_unlit_cutout_culled: bool,
    pub mat_unlit_transparent: bool,
}

pub fn occlusion_cull_chunks(
    settings: Res<RenderDebugSettings>,
    camera_query: Query<(&GlobalTransform, &Projection), With<PlayerCamera>>,
    state: Res<ChunkRenderState>,
    mut chunks: Query<(&ChunkRoot, &mut Visibility)>,
    mut perf: ResMut<RenderPerfStats>,
) {
    let distance_visible_count = chunks
        .iter()
        .filter(|(_, visibility)| !matches!(**visibility, Visibility::Hidden))
        .count() as u32;

    if !settings.occlusion_cull_enabled {
        perf.occlusion_cull_ms = 0.0;
        perf.visible_chunks_after_occlusion = distance_visible_count;
        perf.occluded_chunks = 0;
        return;
    }
    let start = std::time::Instant::now();
    let Ok((cam_transform, projection)) = camera_query.get_single() else {
        perf.occlusion_cull_ms = 0.0;
        perf.visible_chunks_after_occlusion = distance_visible_count;
        perf.occluded_chunks = 0;
        return;
    };
    let (fov_y, aspect, near, far) = camera_fov_params(&settings, projection);
    let tan_y = (fov_y * 0.5).tan() * OCCLUSION_CULL_VERTICAL_FOV_MULTIPLIER;
    let tan_x = tan_y * aspect * OCCLUSION_CULL_HORIZONTAL_FOV_MULTIPLIER;
    let cam_pos = cam_transform.translation();
    let cam_forward = cam_transform.forward();
    let cam_right = cam_transform.right();
    let cam_up = cam_transform.up();
    let camera_chunk = (
        (cam_pos.x / 16.0).floor() as i32,
        (cam_pos.z / 16.0).floor() as i32,
    );

    let mut distance_visible = HashSet::new();
    let mut frustum_candidates = HashSet::new();
    for (chunk, visibility) in &chunks {
        if !matches!(*visibility, Visibility::Hidden) {
            distance_visible.insert(chunk.key);
            if chunk_key_in_coarse_frustum(
                chunk.key,
                cam_pos,
                *cam_forward,
                *cam_right,
                *cam_up,
                tan_x,
                tan_y,
                near,
                far,
            ) {
                frustum_candidates.insert(chunk.key);
            }
        }
    }
    if distance_visible.is_empty() {
        perf.visible_chunks_after_occlusion = 0;
        perf.occluded_chunks = 0;
        perf.occlusion_cull_ms = start.elapsed().as_secs_f32() * 1000.0;
        return;
    }

    let mut keep_visible = HashSet::new();
    if distance_visible.contains(&camera_chunk) {
        frustum_candidates.insert(camera_chunk);
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
        struct PortalNode {
            key: (i32, i32),
            entry_face: Option<ChunkFace>,
        }

        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let start_node = PortalNode {
            key: camera_chunk,
            entry_face: None,
        };
        queue.push_back(start_node);
        visited.insert(start_node);
        keep_visible.insert(camera_chunk);

        let chunk_occlusion_for = |key: (i32, i32)| -> ChunkOcclusionData {
            state
                .entries
                .get(&key)
                .map(|entry| entry.occlusion)
                .unwrap_or_else(ChunkOcclusionData::fully_open)
        };
        let neighbor_for_face = |key: (i32, i32), face: ChunkFace| -> Option<(i32, i32)> {
            match face {
                ChunkFace::NegX => Some((key.0 - 1, key.1)),
                ChunkFace::PosX => Some((key.0 + 1, key.1)),
                ChunkFace::NegZ => Some((key.0, key.1 - 1)),
                ChunkFace::PosZ => Some((key.0, key.1 + 1)),
                ChunkFace::NegY | ChunkFace::PosY => None,
            }
        };

        while let Some(node) = queue.pop_front() {
            let occ = chunk_occlusion_for(node.key);
            let exit_mask = if let Some(entry_face) = node.entry_face {
                if !occ.is_face_open(entry_face) {
                    0
                } else {
                    occ.face_connections[entry_face.index()] & occ.face_open_mask
                }
            } else {
                occ.face_open_mask
            };

            for face in ChunkFace::ALL {
                if (exit_mask & face.bit()) == 0 {
                    continue;
                }
                let Some(neighbor_key) = neighbor_for_face(node.key, face) else {
                    continue;
                };
                if !frustum_candidates.contains(&neighbor_key) {
                    continue;
                }

                let neighbor_occ = chunk_occlusion_for(neighbor_key);
                let enter_face = face.opposite();
                if !neighbor_occ.is_face_open(enter_face) {
                    continue;
                }

                keep_visible.insert(neighbor_key);
                let next = PortalNode {
                    key: neighbor_key,
                    entry_face: Some(enter_face),
                };
                if visited.insert(next) {
                    queue.push_back(next);
                }
            }
        }
    } else {
        // Conservative fallback while crossing chunk-load boundaries:
        // if the camera chunk is not loaded yet, avoid over-culling.
        keep_visible = frustum_candidates.clone();
    }

    for (chunk, mut visibility) in &mut chunks {
        if matches!(*visibility, Visibility::Hidden) {
            continue;
        }
        if keep_visible.contains(&chunk.key) {
            *visibility = Visibility::Visible;
        } else {
            *visibility = Visibility::Hidden;
        }
    }

    perf.visible_chunks_after_occlusion = keep_visible.len() as u32;
    perf.occluded_chunks = distance_visible.len().saturating_sub(keep_visible.len()) as u32;
    perf.occlusion_cull_ms = start.elapsed().as_secs_f32() * 1000.0;
}

#[allow(clippy::too_many_arguments)]
fn chunk_key_in_coarse_frustum(
    chunk_key: (i32, i32),
    cam_pos: Vec3,
    cam_forward: Vec3,
    cam_right: Vec3,
    cam_up: Vec3,
    tan_x: f32,
    tan_y: f32,
    near: f32,
    far: f32,
) -> bool {
    let base_x = (chunk_key.0 * 16 + 8) as f32;
    let base_z = (chunk_key.1 * 16 + 8) as f32;
    for sample_y in OCCLUSION_CULL_Y_SAMPLES {
        let sample = Vec3::new(base_x, sample_y, base_z);
        let to_sample = sample - cam_pos;
        let z = to_sample.dot(cam_forward);
        let x = to_sample.dot(cam_right).abs();
        let y = to_sample.dot(cam_up).abs();
        if z < near - OCCLUSION_CULL_RADIUS - OCCLUSION_CULL_FRUSTUM_PAD
            || z > far + OCCLUSION_CULL_RADIUS + OCCLUSION_CULL_FRUSTUM_PAD
        {
            continue;
        }
        if x <= z * tan_x + OCCLUSION_CULL_RADIUS + OCCLUSION_CULL_FRUSTUM_PAD
            && y <= z * tan_y + OCCLUSION_CULL_RADIUS + OCCLUSION_CULL_FRUSTUM_PAD
        {
            return true;
        }
    }
    false
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
    mut settings: ResMut<RenderDebugSettings>,
    mut state: ResMut<MeshingToggleState>,
    store: Res<ChunkStore>,
    async_mesh: Res<MeshAsyncResources>,
    mut in_flight: ResMut<MeshInFlight>,
    assets: Res<ChunkRenderAssets>,
) {
    if settings.use_greedy_meshing == state.last_use_greedy
        && settings.voxel_ao_enabled == state.last_voxel_ao_enabled
        && settings.voxel_ao_cutout == state.last_voxel_ao_cutout
        && (settings.voxel_ao_strength - state.last_voxel_ao_strength).abs() < 0.001
        && settings.barrier_billboard == state.last_barrier_billboard
        && !settings.force_remesh
    {
        return;
    }
    state.last_use_greedy = settings.use_greedy_meshing;
    state.last_voxel_ao_enabled = settings.voxel_ao_enabled;
    state.last_voxel_ao_cutout = settings.voxel_ao_cutout;
    state.last_voxel_ao_strength = settings.voxel_ao_strength;
    state.last_barrier_billboard = settings.barrier_billboard;
    settings.force_remesh = false;
    in_flight.chunks.clear();
    for key in store.chunks.keys().copied() {
        let snapshot = snapshot_for_chunk(&store, key);
        let job = MeshJob {
            chunk_key: key,
            snapshot,
            use_greedy: settings.use_greedy_meshing,
            leaf_depth_layer_faces: true,
            voxel_ao_enabled: settings.voxel_ao_enabled,
            voxel_ao_strength: settings.voxel_ao_strength,
            voxel_ao_cutout: settings.voxel_ao_cutout,
            barrier_billboard: settings.barrier_billboard,
            texture_mapping: assets.texture_mapping.clone(),
            biome_tints: assets.biome_tints.clone(),
        };
        if async_mesh.job_tx.send(job).is_ok() {
            in_flight.chunks.insert(key);
        }
    }
}

pub fn refresh_render_state_on_mode_change(
    mut settings: ResMut<RenderDebugSettings>,
    mut last_mode: Local<Option<(u8, bool)>>,
) {
    let mode = (
        settings.shader_quality_mode,
        settings.enable_pbr_terrain_lighting,
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
    let tan_x = tan_y * aspect * MANUAL_CULL_HORIZONTAL_FOV_MULTIPLIER;
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
        let cull_pad = MANUAL_CULL_PAD_BASE
            + if settings.shadows_enabled {
                MANUAL_CULL_PAD_SHADOW_EXTRA
            } else {
                0.0
            };
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
        if z < -radius - cull_pad * 2.0 {
            *visibility = Visibility::Hidden;
            continue;
        }
        let x = to_center.dot(*right).abs();
        let y = to_center.dot(*up).abs();
        let visible = x <= z * tan_x + radius
            && y <= z * tan_y + radius
            && z <= far + radius + cull_pad * 2.0
            && z >= near - radius - cull_pad * 2.0;
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
