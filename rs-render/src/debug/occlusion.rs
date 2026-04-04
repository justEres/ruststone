use std::collections::{HashSet, VecDeque};

use bevy::prelude::*;

use crate::chunk::{ChunkFace, ChunkOcclusionData, ChunkRenderState};
use crate::components::{ChunkRoot, Player, PlayerCamera};

use super::{RenderDebugSettings, RenderPerfStats};

const OCCLUSION_CULL_HORIZONTAL_FOV_MULTIPLIER: f32 = 1.85;
const OCCLUSION_CULL_VERTICAL_FOV_MULTIPLIER: f32 = 1.60;
const OCCLUSION_CULL_RADIUS: f32 = 20.0;
const OCCLUSION_CULL_FRUSTUM_PAD: f32 = 24.0;
const OCCLUSION_CULL_Y_SAMPLES: [f32; 5] = [8.0, 64.0, 128.0, 192.0, 248.0];

#[derive(Resource, Default)]
pub struct OcclusionCullCache {
    pub anchor_chunk: Option<(i32, i32)>,
    pub camera_chunk: Option<(i32, i32)>,
    pub cull_pos: Option<Vec3>,
    pub cull_forward: Option<Vec3>,
    pub occlusion_revision: u64,
    pub guard_radius: i32,
    pub visible_chunks_after_occlusion: u32,
    pub occluded_chunks: u32,
}

pub fn occlusion_cull_chunks(
    settings: Res<RenderDebugSettings>,
    camera_query: Query<(&GlobalTransform, &Projection), With<PlayerCamera>>,
    player_query: Query<&GlobalTransform, With<Player>>,
    state: Res<ChunkRenderState>,
    mut chunks: Query<(&ChunkRoot, &mut Visibility)>,
    mut perf: ResMut<RenderPerfStats>,
    mut cache: ResMut<OcclusionCullCache>,
) {
    let guard_radius = settings.cull_guard_chunk_radius.clamp(0, 5);
    let distance_visible_count = chunks
        .iter()
        .filter(|(_, visibility)| !matches!(**visibility, Visibility::Hidden))
        .count() as u32;

    if !settings.occlusion_cull_enabled {
        perf.occlusion_cull_ms = 0.0;
        perf.visible_chunks_after_occlusion = distance_visible_count;
        perf.occluded_chunks = 0;
        *cache = OcclusionCullCache::default();
        return;
    }
    let start = std::time::Instant::now();
    let Ok((cam_transform, projection)) = camera_query.get_single() else {
        perf.occlusion_cull_ms = 0.0;
        perf.visible_chunks_after_occlusion = distance_visible_count;
        perf.occluded_chunks = 0;
        *cache = OcclusionCullCache::default();
        return;
    };
    let (fov_y, aspect, near, far) = camera_fov_params(&settings, projection);
    let tan_y = (fov_y * 0.5).tan() * OCCLUSION_CULL_VERTICAL_FOV_MULTIPLIER;
    let tan_x = tan_y * aspect * OCCLUSION_CULL_HORIZONTAL_FOV_MULTIPLIER;
    let cam_pos = cam_transform.translation();
    let camera_chunk = (
        (cam_pos.x / 16.0).floor() as i32,
        (cam_pos.z / 16.0).floor() as i32,
    );
    let (cull_pos, cull_forward, cull_right, cull_up, anchor_chunk) =
        if settings.occlusion_anchor_player {
            player_query
                .get_single()
                .map(|player_transform| {
                    let p = player_transform.translation();
                    (
                        p,
                        player_transform.forward(),
                        player_transform.right(),
                        player_transform.up(),
                        ((p.x / 16.0).floor() as i32, (p.z / 16.0).floor() as i32),
                    )
                })
                .unwrap_or((
                    cam_pos,
                    cam_transform.forward(),
                    cam_transform.right(),
                    cam_transform.up(),
                    camera_chunk,
                ))
        } else {
            (
                cam_pos,
                cam_transform.forward(),
                cam_transform.right(),
                cam_transform.up(),
                camera_chunk,
            )
        };

    let stable_camera = cache.camera_chunk == Some(camera_chunk)
        && cache.anchor_chunk == Some(anchor_chunk)
        && cache.guard_radius == guard_radius
        && cache.occlusion_revision == state.occlusion_revision
        && cache
            .cull_pos
            .map(|prev| prev.distance_squared(cull_pos) <= 4.0)
            .unwrap_or(false)
        && cache
            .cull_forward
            .map(|prev| prev.dot(*cull_forward) >= 0.995)
            .unwrap_or(false);
    if stable_camera {
        perf.occlusion_cull_ms = 0.0;
        perf.visible_chunks_after_occlusion = cache.visible_chunks_after_occlusion;
        perf.occluded_chunks = cache.occluded_chunks;
        return;
    }

    let mut distance_visible = HashSet::new();
    let mut frustum_candidates = HashSet::new();
    let mut guard_visible = HashSet::new();
    for (chunk, visibility) in &chunks {
        if !matches!(*visibility, Visibility::Hidden) {
            distance_visible.insert(chunk.key);
            let near_guard = (chunk.key.0 - anchor_chunk.0).abs() <= guard_radius
                && (chunk.key.1 - anchor_chunk.1).abs() <= guard_radius;
            if near_guard {
                guard_visible.insert(chunk.key);
                frustum_candidates.insert(chunk.key);
                continue;
            }
            if chunk_key_in_coarse_frustum(
                chunk.key,
                cull_pos,
                *cull_forward,
                *cull_right,
                *cull_up,
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
        cache.anchor_chunk = Some(anchor_chunk);
        cache.camera_chunk = Some(camera_chunk);
        cache.cull_pos = Some(cull_pos);
        cache.cull_forward = Some(*cull_forward);
        cache.occlusion_revision = state.occlusion_revision;
        cache.guard_radius = guard_radius;
        cache.visible_chunks_after_occlusion = 0;
        cache.occluded_chunks = 0;
        return;
    }

    let mut keep_visible = HashSet::new();
    keep_visible.extend(guard_visible.iter().copied());
    if distance_visible.contains(&anchor_chunk) {
        frustum_candidates.insert(anchor_chunk);
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
        struct PortalNode {
            key: (i32, i32),
            entry_face: Option<ChunkFace>,
        }

        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let start_node = PortalNode {
            key: anchor_chunk,
            entry_face: None,
        };
        queue.push_back(start_node);
        visited.insert(start_node);
        keep_visible.insert(anchor_chunk);

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
    cache.anchor_chunk = Some(anchor_chunk);
    cache.camera_chunk = Some(camera_chunk);
    cache.cull_pos = Some(cull_pos);
    cache.cull_forward = Some(*cull_forward);
    cache.occlusion_revision = state.occlusion_revision;
    cache.guard_radius = guard_radius;
    cache.visible_chunks_after_occlusion = perf.visible_chunks_after_occlusion;
    cache.occluded_chunks = perf.occluded_chunks;
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

pub(crate) fn camera_fov_params(
    settings: &RenderDebugSettings,
    projection: &Projection,
) -> (f32, f32, f32, f32) {
    let (mut fov_y, mut aspect, mut near, mut far) = match projection {
        Projection::Perspective(p) => (p.fov, p.aspect_ratio, p.near, p.far),
        _ => (settings.fov_deg.to_radians(), 1.0, 0.1, 1000.0),
    };
    fov_y = fov_y.max(settings.fov_deg.to_radians());
    if settings.frustum_fov_debug {
        fov_y = settings.frustum_fov_deg.max(1.0).to_radians();
    }
    fov_y = (fov_y * 1.40).min(std::f32::consts::PI - 0.01);
    aspect = aspect.max(0.01);
    near = near.max(0.01);
    far = far.max(near + 0.01);
    (fov_y, aspect, near, far)
}
