use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn add_cross_plant(
    batch: &mut MeshBatch,
    snapshot: &ChunkColumnSnapshot,
    texture_mapping: &AtlasBlockMapping,
    biome_tints: &BiomeTintResolver,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    block_id: u16,
    tint: BiomeTint,
    voxel_ao_enabled: bool,
    voxel_ao_strength: f32,
    voxel_ao_cutout: bool,
) {
    let texture_index = texture_mapping.texture_index_for_state(block_id, Face::PosZ);
    let tile_origin = atlas_tile_origin(texture_index);
    let uvs = uv_for_texture();
    let mut color = tint_color(
        block_id,
        tint,
        snapshot,
        chunk_x,
        chunk_z,
        x,
        y,
        z,
        biome_tints,
        None,
    );
    if let Some(tint_rgb) =
        cross_vegetation_biome_tint(block_id, snapshot, chunk_x, chunk_z, x, y, z, tint)
    {
        color[0] = tint_rgb[0];
        color[1] = tint_rgb[1];
        color[2] = tint_rgb[2];
    }
    let shade = if can_apply_vertex_shading(block_id, voxel_ao_cutout) {
        cross_plant_shade(
            snapshot,
            chunk_x,
            chunk_z,
            x,
            y,
            z,
            block_id,
            voxel_ao_enabled,
            voxel_ao_strength,
        )
    } else if should_apply_prebaked_shade(block_id) {
        block_light_factor(snapshot, chunk_x, chunk_z, x, y, z)
    } else {
        1.0
    };
    color[0] *= shade;
    color[1] *= shade;
    color[2] *= shade;
    let data = batch.data_for(block_id);

    let x0 = x as f32;
    let y0 = y as f32;
    let z0 = z as f32;

    let cross_normal_lift = |n: Vec3| -> [f32; 3] {
        let lifted = Vec3::new(n.x, 0.38, n.z).normalize_or_zero();
        [lifted.x, lifted.y, lifted.z]
    };

    let normal_a = cross_normal_lift(Vec3::new(1.0, 0.0, 1.0));
    let a = [
        [x0 + 0.0, y0 + 0.0, z0 + 0.0],
        [x0 + 1.0, y0 + 0.0, z0 + 1.0],
        [x0 + 1.0, y0 + 1.0, z0 + 1.0],
        [x0 + 0.0, y0 + 1.0, z0 + 0.0],
    ];
    add_double_sided_quad(data, a, normal_a, uvs, tile_origin, color);

    let normal_b = cross_normal_lift(Vec3::new(-1.0, 0.0, 1.0));
    let b = [
        [x0 + 1.0, y0 + 0.0, z0 + 0.0],
        [x0 + 0.0, y0 + 0.0, z0 + 1.0],
        [x0 + 0.0, y0 + 1.0, z0 + 1.0],
        [x0 + 1.0, y0 + 1.0, z0 + 0.0],
    ];
    add_double_sided_quad(data, b, normal_b, uvs, tile_origin, color);
}

fn cross_vegetation_biome_tint(
    block_state: u16,
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    tint: BiomeTint,
) -> Option<[f32; 3]> {
    let id = block_type(block_state);
    match id {
        6 => Some([tint.foliage[0], tint.foliage[1], tint.foliage[2]]),
        31 => Some([tint.grass[0], tint.grass[1], tint.grass[2]]),
        83 => Some([tint.grass[0], tint.grass[1], tint.grass[2]]),
        175 => {
            let meta = block_meta(block_state);
            let lower_meta = if (meta & 0x8) != 0 {
                block_meta(block_at(snapshot, chunk_x, chunk_z, x, y - 1, z))
            } else {
                meta
            };
            if matches!(lower_meta & 0x7, 2 | 3) {
                Some([tint.grass[0], tint.grass[1], tint.grass[2]])
            } else {
                None
            }
        }
        _ => None,
    }
}

fn add_double_sided_quad(
    data: &mut MeshData,
    verts: [[f32; 3]; 4],
    normal: [f32; 3],
    uvs: [[f32; 2]; 4],
    tile_origin: [f32; 2],
    color: [f32; 4],
) {
    let base = data.positions.len() as u32;
    for i in 0..4 {
        data.push_pos(verts[i]);
        data.normals.push(normal);
        data.uvs.push(uvs[i]);
        data.uvs_b.push(tile_origin);
        data.colors.push(color);
    }
    data.indices
        .extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);

    let back_base = data.positions.len() as u32;
    for i in 0..4 {
        data.push_pos(verts[i]);
        data.normals.push(normal);
        data.uvs.push(uvs[i]);
        data.uvs_b.push(tile_origin);
        data.colors.push(color);
    }
    data.indices.extend_from_slice(&[
        back_base,
        back_base + 1,
        back_base + 2,
        back_base,
        back_base + 2,
        back_base + 3,
    ]);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn add_box(
    batch: &mut MeshBatch,
    neighbor_ctx: Option<(&ChunkColumnSnapshot, i32, i32, i32, i32, i32, u16)>,
    texture_mapping: &AtlasBlockMapping,
    biome_tints: &BiomeTintResolver,
    x: i32,
    y: i32,
    z: i32,
    min: [f32; 3],
    max: [f32; 3],
    block_id: u16,
    tint: BiomeTint,
) {
    let faces = [
        (
            Face::PosX,
            1,
            0,
            0,
            [1.0, 0.0, 0.0],
            [
                [max[0], min[1], min[2]],
                [max[0], min[1], max[2]],
                [max[0], max[1], max[2]],
                [max[0], max[1], min[2]],
            ],
            max[0] >= 1.0,
        ),
        (
            Face::NegX,
            -1,
            0,
            0,
            [-1.0, 0.0, 0.0],
            [
                [min[0], min[1], max[2]],
                [min[0], min[1], min[2]],
                [min[0], max[1], min[2]],
                [min[0], max[1], max[2]],
            ],
            min[0] <= 0.0,
        ),
        (
            Face::PosY,
            0,
            1,
            0,
            [0.0, 1.0, 0.0],
            [
                [min[0], max[1], min[2]],
                [max[0], max[1], min[2]],
                [max[0], max[1], max[2]],
                [min[0], max[1], max[2]],
            ],
            max[1] >= 1.0,
        ),
        (
            Face::NegY,
            0,
            -1,
            0,
            [0.0, -1.0, 0.0],
            [
                [min[0], min[1], max[2]],
                [max[0], min[1], max[2]],
                [max[0], min[1], min[2]],
                [min[0], min[1], min[2]],
            ],
            min[1] <= 0.0,
        ),
        (
            Face::PosZ,
            0,
            0,
            1,
            [0.0, 0.0, 1.0],
            [
                [max[0], min[1], max[2]],
                [min[0], min[1], max[2]],
                [min[0], max[1], max[2]],
                [max[0], max[1], max[2]],
            ],
            max[2] >= 1.0,
        ),
        (
            Face::NegZ,
            0,
            0,
            -1,
            [0.0, 0.0, -1.0],
            [
                [min[0], min[1], min[2]],
                [max[0], min[1], min[2]],
                [max[0], max[1], min[2]],
                [min[0], max[1], min[2]],
            ],
            min[2] <= 0.0,
        ),
    ];

    for (face, dx, dy, dz, normal, verts, boundary_face) in faces {
        if let Some((snapshot, chunk_x, chunk_z, bx, by, bz, block_id_for_cull)) = neighbor_ctx
            && boundary_face
        {
            let neighbor = block_at(snapshot, chunk_x, chunk_z, bx + dx, by + dy, bz + dz);
            if face_is_occluded(block_id_for_cull, neighbor, true) {
                continue;
            }
        }

        let texture_index = texture_mapping.texture_index_for_state(block_id, face);
        let data = batch.data_for(block_id);
        let base_index = data.positions.len() as u32;
        for vert in verts {
            data.push_pos([vert[0] + x as f32, vert[1] + y as f32, vert[2] + z as f32]);
            data.normals.push(normal);
        }
        let uvs = box_face_uvs(face, min, max);
        data.uvs.extend_from_slice(&uvs);
        let tile_origin = atlas_tile_origin(texture_index);
        data.uvs_b.extend_from_slice(&[tile_origin; 4]);
        let mut color = if let Some((snapshot, chunk_x, chunk_z, bx, by, bz, _)) = neighbor_ctx {
            tint_color(
                block_id,
                tint,
                snapshot,
                chunk_x,
                chunk_z,
                bx,
                by,
                bz,
                biome_tints,
                None,
            )
        } else {
            tint_color_untargeted(block_id, tint, None)
        };
        if let Some((snapshot, chunk_x, chunk_z, bx, by, bz, _)) = neighbor_ctx {
            let shade = if should_apply_prebaked_shade(block_id) {
                face_light_factor(snapshot, chunk_x, chunk_z, bx, by, bz, face)
            } else {
                1.0
            };
            color[0] *= shade;
            color[1] *= shade;
            color[2] *= shade;
        }
        data.colors.extend_from_slice(&[color, color, color, color]);
        data.indices.extend_from_slice(&[
            base_index,
            base_index + 2,
            base_index + 1,
            base_index,
            base_index + 3,
            base_index + 2,
        ]);
    }
}

pub(super) fn box_face_uvs(face: Face, min: [f32; 3], max: [f32; 3]) -> [[f32; 2]; 4] {
    match face {
        Face::PosX => [
            [min[2], 1.0 - min[1]],
            [max[2], 1.0 - min[1]],
            [max[2], 1.0 - max[1]],
            [min[2], 1.0 - max[1]],
        ],
        Face::NegX => [
            [1.0 - max[2], 1.0 - min[1]],
            [1.0 - min[2], 1.0 - min[1]],
            [1.0 - min[2], 1.0 - max[1]],
            [1.0 - max[2], 1.0 - max[1]],
        ],
        Face::PosY => [
            [min[0], min[2]],
            [max[0], min[2]],
            [max[0], max[2]],
            [min[0], max[2]],
        ],
        Face::NegY => [
            [min[0], 1.0 - max[2]],
            [max[0], 1.0 - max[2]],
            [max[0], 1.0 - min[2]],
            [min[0], 1.0 - min[2]],
        ],
        Face::PosZ => [
            [1.0 - max[0], 1.0 - min[1]],
            [1.0 - min[0], 1.0 - min[1]],
            [1.0 - min[0], 1.0 - max[1]],
            [1.0 - max[0], 1.0 - max[1]],
        ],
        Face::NegZ => [
            [min[0], 1.0 - min[1]],
            [max[0], 1.0 - min[1]],
            [max[0], 1.0 - max[1]],
            [min[0], 1.0 - max[1]],
        ],
    }
}

pub(super) fn is_custom_block(block_id: u16) -> bool {
    matches!(
        block_model_kind(block_type(block_id)),
        BlockModelKind::Cross
            | BlockModelKind::Slab
            | BlockModelKind::Stairs
            | BlockModelKind::Fence
            | BlockModelKind::Pane
            | BlockModelKind::TorchLike
            | BlockModelKind::Custom
    )
}
