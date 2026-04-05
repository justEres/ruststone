use super::*;

pub(super) fn is_ao_occluder(block_state: u16) -> bool {
    let id = block_type(block_state);
    if id == 0 || id == 78 || is_transparent_block(id) {
        return false;
    }
    if is_leaves_block(id) || is_alpha_cutout_cube(id) {
        return false;
    }
    !matches!(
        block_model_kind(id),
        BlockModelKind::Cross
            | BlockModelKind::Pane
            | BlockModelKind::TorchLike
            | BlockModelKind::Custom
    )
}

fn ao_factor(side1: bool, side2: bool, corner: bool) -> f32 {
    let level = if side1 && side2 {
        0
    } else {
        3 - (side1 as u8 + side2 as u8 + corner as u8)
    };
    match level {
        0 => 0.56,
        1 => 0.70,
        2 => 0.84,
        _ => 1.0,
    }
}

fn is_block_normal_cube_for_ao(block_state: u16) -> bool {
    let id = block_type(block_state);
    if id == 0 || is_liquid(block_state) || is_transparent_block(id) || is_alpha_cutout_cube(id) {
        return false;
    }
    !matches!(
        block_model_kind(id),
        BlockModelKind::Cross
            | BlockModelKind::Slab
            | BlockModelKind::Stairs
            | BlockModelKind::Fence
            | BlockModelKind::Pane
            | BlockModelKind::TorchLike
            | BlockModelKind::Custom
    )
}

fn block_ao_light_value(block_state: u16) -> f32 {
    if is_block_normal_cube_for_ao(block_state) {
        0.2
    } else {
        1.0
    }
}

fn ao_occlusion_weight(block_state: u16) -> f32 {
    let id = block_type(block_state);
    if id == 0 || id == 78 || is_transparent_block(id) {
        return 0.0;
    }
    if is_leaves_block(id) {
        return 0.45;
    }
    if is_alpha_cutout_cube(id) {
        return 0.20;
    }
    if matches!(
        block_model_kind(id),
        BlockModelKind::Cross
            | BlockModelKind::Pane
            | BlockModelKind::TorchLike
            | BlockModelKind::Custom
    ) {
        return 0.0;
    }
    1.0
}

fn weighted_ao_factor(side1: f32, side2: f32, corner: f32) -> f32 {
    let full = ao_factor(side1 > 0.0, side2 > 0.0, corner > 0.0);
    let mut weight_sum = 0.0;
    let mut contributors = 0.0;
    for weight in [side1, side2, corner] {
        if weight > 0.0 {
            weight_sum += weight;
            contributors += 1.0;
        }
    }
    if contributors <= 0.0 {
        return 1.0;
    }
    let avg_weight = (weight_sum / contributors).clamp(0.0, 1.0);
    1.0 - (1.0 - full) * avg_weight
}

fn quantize_shade_4bit(shade: f32) -> u16 {
    (shade.clamp(0.0, 1.0) * 15.0).round() as u16
}

#[allow(clippy::too_many_arguments)]
pub(super) fn face_shade_signature(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    face: Face,
    block_id: u16,
    voxel_ao_enabled: bool,
    voxel_ao_strength: f32,
    voxel_ao_cutout: bool,
    voxel_ao_foliage_boost: f32,
    vanilla_bake: VanillaBakeSettings,
) -> u16 {
    let verts = face_vertices(face);
    let mut packed = 0u16;
    for (idx, vert) in verts.into_iter().enumerate() {
        let shade = compute_vertex_shade(
            snapshot,
            chunk_x,
            chunk_z,
            x,
            y,
            z,
            face,
            vert,
            block_id,
            voxel_ao_enabled,
            voxel_ao_strength,
            voxel_ao_cutout,
            voxel_ao_foliage_boost,
            vanilla_bake,
        );
        packed |= quantize_shade_4bit(shade) << (idx * 4);
    }
    packed
}

pub(super) fn face_vertex_light_ao(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    face: Face,
    vertex: [f32; 3],
) -> (f32, f32, f32) {
    let (nx, ny, nz, axis_a, axis_b) = match face {
        Face::PosX => (1, 0, 0, 1usize, 2usize),
        Face::NegX => (-1, 0, 0, 1usize, 2usize),
        Face::PosY => (0, 1, 0, 0usize, 2usize),
        Face::NegY => (0, -1, 0, 0usize, 2usize),
        Face::PosZ => (0, 0, 1, 0usize, 1usize),
        Face::NegZ => (0, 0, -1, 0usize, 1usize),
    };

    let signs = |coord: f32| if coord <= 0.0 { -1 } else { 1 };
    let mut delta = [0i32; 3];
    delta[axis_a] = signs(vertex[axis_a]);
    delta[axis_b] = signs(vertex[axis_b]);

    let base = (x + nx, y + ny, z + nz);
    let s1 = (base.0 + delta[0], base.1 + delta[1], base.2 + delta[2]);
    let mut side1 = [base.0, base.1, base.2];
    side1[axis_a] += delta[axis_a];
    let mut side2 = [base.0, base.1, base.2];
    side2[axis_b] += delta[axis_b];

    let occ_side1 = is_ao_occluder(block_at(
        snapshot, chunk_x, chunk_z, side1[0], side1[1], side1[2],
    ));
    let occ_side2 = is_ao_occluder(block_at(
        snapshot, chunk_x, chunk_z, side2[0], side2[1], side2[2],
    ));
    let occ_corner = is_ao_occluder(block_at(snapshot, chunk_x, chunk_z, s1.0, s1.1, s1.2));
    let ao = ao_factor(occ_side1, occ_side2, occ_corner);

    let l0 = light_at(snapshot, chunk_x, chunk_z, base.0, base.1, base.2);
    let l1 = light_at(snapshot, chunk_x, chunk_z, side1[0], side1[1], side1[2]);
    let l2 = light_at(snapshot, chunk_x, chunk_z, side2[0], side2[1], side2[2]);
    let l3 = light_at(snapshot, chunk_x, chunk_z, s1.0, s1.1, s1.2);
    let sky_level = (f32::from(l0.sky) + f32::from(l1.sky) + f32::from(l2.sky) + f32::from(l3.sky))
        * 0.25;
    let block_level =
        (f32::from(l0.block) + f32::from(l1.block) + f32::from(l2.block) + f32::from(l3.block))
            * 0.25;
    (ao, sky_level, block_level)
}

pub(super) fn face_vertex_weighted_ao(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    face: Face,
    vertex: [f32; 3],
) -> f32 {
    let (nx, ny, nz, axis_a, axis_b) = match face {
        Face::PosX => (1, 0, 0, 1usize, 2usize),
        Face::NegX => (-1, 0, 0, 1usize, 2usize),
        Face::PosY => (0, 1, 0, 0usize, 2usize),
        Face::NegY => (0, -1, 0, 0usize, 2usize),
        Face::PosZ => (0, 0, 1, 0usize, 1usize),
        Face::NegZ => (0, 0, -1, 0usize, 1usize),
    };

    let signs = |coord: f32| if coord <= 0.0 { -1 } else { 1 };
    let mut delta = [0i32; 3];
    delta[axis_a] = signs(vertex[axis_a]);
    delta[axis_b] = signs(vertex[axis_b]);

    let base = (x + nx, y + ny, z + nz);
    let s1 = (base.0 + delta[0], base.1 + delta[1], base.2 + delta[2]);
    let mut side1 = [base.0, base.1, base.2];
    side1[axis_a] += delta[axis_a];
    let mut side2 = [base.0, base.1, base.2];
    side2[axis_b] += delta[axis_b];

    weighted_ao_factor(
        ao_occlusion_weight(block_at(snapshot, chunk_x, chunk_z, side1[0], side1[1], side1[2])),
        ao_occlusion_weight(block_at(snapshot, chunk_x, chunk_z, side2[0], side2[1], side2[2])),
        ao_occlusion_weight(block_at(snapshot, chunk_x, chunk_z, s1.0, s1.1, s1.2)),
    )
}

fn face_vertex_vanilla_ao(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    face: Face,
    vertex: [f32; 3],
) -> f32 {
    let (nx, ny, nz, axis_a, axis_b) = match face {
        Face::PosX => (1, 0, 0, 1usize, 2usize),
        Face::NegX => (-1, 0, 0, 1usize, 2usize),
        Face::PosY => (0, 1, 0, 0usize, 2usize),
        Face::NegY => (0, -1, 0, 0usize, 2usize),
        Face::PosZ => (0, 0, 1, 0usize, 1usize),
        Face::NegZ => (0, 0, -1, 0usize, 1usize),
    };

    let signs = |coord: f32| if coord <= 0.0 { -1 } else { 1 };
    let mut delta = [0i32; 3];
    delta[axis_a] = signs(vertex[axis_a]);
    delta[axis_b] = signs(vertex[axis_b]);

    let base = (x + nx, y + ny, z + nz);
    let s1 = (base.0 + delta[0], base.1 + delta[1], base.2 + delta[2]);
    let mut side1 = [base.0, base.1, base.2];
    side1[axis_a] += delta[axis_a];
    let mut side2 = [base.0, base.1, base.2];
    side2[axis_b] += delta[axis_b];

    let base_ao =
        block_ao_light_value(block_at(snapshot, chunk_x, chunk_z, base.0, base.1, base.2));
    let side1_ao =
        block_ao_light_value(block_at(snapshot, chunk_x, chunk_z, side1[0], side1[1], side1[2]));
    let side2_ao =
        block_ao_light_value(block_at(snapshot, chunk_x, chunk_z, side2[0], side2[1], side2[2]));
    let corner_ao = block_ao_light_value(block_at(snapshot, chunk_x, chunk_z, s1.0, s1.1, s1.2));
    (base_ao + side1_ao + side2_ao + corner_ao) * 0.25
}

pub(super) fn averaged_face_vanilla_ao(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    face: Face,
) -> f32 {
    let mut total = 0.0;
    for vertex in face_vertices(face) {
        total += face_vertex_vanilla_ao(snapshot, chunk_x, chunk_z, x, y, z, face, vertex);
    }
    total * 0.25
}

fn averaged_face_weighted_ao(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    face: Face,
) -> f32 {
    let mut total = 0.0;
    for vertex in face_vertices(face) {
        total += face_vertex_weighted_ao(snapshot, chunk_x, chunk_z, x, y, z, face, vertex);
    }
    total * 0.25
}

pub(super) fn apply_ao_strength(ao: f32, strength: f32) -> f32 {
    let strength = strength.clamp(0.0, 2.0);
    1.0 - strength + ao.clamp(0.0, 1.0) * strength
}

#[allow(clippy::too_many_arguments)]
pub(super) fn greedy_face_corner_shades(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    face: Face,
    axis: i32,
    base_y: i32,
    quad: &GreedyQuad,
    block_id: u16,
    voxel_ao_enabled: bool,
    voxel_ao_strength: f32,
    voxel_ao_cutout: bool,
    voxel_ao_foliage_boost: f32,
    vanilla_bake: VanillaBakeSettings,
) -> [f32; 4] {
    let x0 = quad.x as i32;
    let x1 = quad.x as i32 + quad.w as i32 - 1;
    let y0 = base_y + quad.y as i32;
    let y1 = base_y + quad.y as i32 + quad.h as i32 - 1;
    let z0 = quad.y as i32;
    let z1 = quad.y as i32 + quad.h as i32 - 1;

    let sample = |sx: i32, sy: i32, sz: i32, vx: f32, vy: f32, vz: f32| {
        compute_vertex_shade(
            snapshot,
            chunk_x,
            chunk_z,
            sx,
            sy,
            sz,
            face,
            [vx, vy, vz],
            block_id,
            voxel_ao_enabled,
            voxel_ao_strength,
            voxel_ao_cutout,
            voxel_ao_foliage_boost,
            vanilla_bake,
        )
    };

    match face {
        Face::PosY => [
            sample(x0, base_y + axis, z0, 0.0, 1.0, 0.0),
            sample(x1, base_y + axis, z0, 1.0, 1.0, 0.0),
            sample(x1, base_y + axis, z1, 1.0, 1.0, 1.0),
            sample(x0, base_y + axis, z1, 0.0, 1.0, 1.0),
        ],
        Face::NegY => [
            sample(x0, base_y + axis, z1, 0.0, 0.0, 1.0),
            sample(x1, base_y + axis, z1, 1.0, 0.0, 1.0),
            sample(x1, base_y + axis, z0, 1.0, 0.0, 0.0),
            sample(x0, base_y + axis, z0, 0.0, 0.0, 0.0),
        ],
        Face::PosX => [
            sample(axis, y0, x0, 1.0, 0.0, 0.0),
            sample(axis, y0, x1, 1.0, 0.0, 1.0),
            sample(axis, y1, x1, 1.0, 1.0, 1.0),
            sample(axis, y1, x0, 1.0, 1.0, 0.0),
        ],
        Face::NegX => [
            sample(axis, y0, x1, 0.0, 0.0, 1.0),
            sample(axis, y0, x0, 0.0, 0.0, 0.0),
            sample(axis, y1, x0, 0.0, 1.0, 0.0),
            sample(axis, y1, x1, 0.0, 1.0, 1.0),
        ],
        Face::PosZ => [
            sample(x1, y0, axis, 1.0, 0.0, 1.0),
            sample(x0, y0, axis, 0.0, 0.0, 1.0),
            sample(x0, y1, axis, 0.0, 1.0, 1.0),
            sample(x1, y1, axis, 1.0, 1.0, 1.0),
        ],
        Face::NegZ => [
            sample(x0, y0, axis, 0.0, 0.0, 0.0),
            sample(x1, y0, axis, 1.0, 0.0, 0.0),
            sample(x1, y1, axis, 1.0, 1.0, 0.0),
            sample(x0, y1, axis, 0.0, 1.0, 0.0),
        ],
    }
}

pub(super) fn cross_plant_shade(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    block_id: u16,
    voxel_ao_enabled: bool,
    voxel_ao_strength: f32,
    voxel_ao_foliage_boost: f32,
) -> f32 {
    let mut shade = block_light_factor(snapshot, chunk_x, chunk_z, x, y, z).max(0.34);
    if voxel_ao_enabled {
        let side_ao = [
            averaged_face_weighted_ao(snapshot, chunk_x, chunk_z, x, y, z, Face::PosX),
            averaged_face_weighted_ao(snapshot, chunk_x, chunk_z, x, y, z, Face::NegX),
            averaged_face_weighted_ao(snapshot, chunk_x, chunk_z, x, y, z, Face::PosZ),
            averaged_face_weighted_ao(snapshot, chunk_x, chunk_z, x, y, z, Face::NegZ),
        ]
        .into_iter()
        .sum::<f32>()
            * 0.25;
        let ao_strength = if is_softened_vanilla_foliage(block_id) {
            voxel_ao_strength * 0.75 * voxel_ao_foliage_boost.clamp(0.5, 4.0)
        } else {
            voxel_ao_strength * 0.55 * voxel_ao_foliage_boost.clamp(0.5, 4.0)
        };
        shade *= apply_ao_strength(side_ao, ao_strength);
    }
    shade.clamp(0.0, 1.0)
}
