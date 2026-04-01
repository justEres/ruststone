use super::*;

pub(super) fn build_chunk_mesh_culled(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    leaf_depth_layer_faces: bool,
    voxel_ao_enabled: bool,
    voxel_ao_strength: f32,
    voxel_ao_cutout: bool,
    barrier_billboard: bool,
    vanilla_bake: VanillaBakeSettings,
    texture_mapping: &AtlasBlockMapping,
    biome_tints: &BiomeTintResolver,
) -> MeshBatch {
    let mut batch = MeshBatch::default();

    let Some(column) = snapshot.columns.get(&(chunk_x, chunk_z)) else {
        return batch;
    };

    for (section_y, section_opt) in column.sections.iter().enumerate() {
        let Some(section_blocks) = section_opt else {
            continue;
        };
        let base_y = section_y as i32 * SECTION_HEIGHT;
        for y in 0..SECTION_HEIGHT {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let idx = (y * CHUNK_SIZE * CHUNK_SIZE + z * CHUNK_SIZE + x) as usize;
                    let block_id = section_blocks[idx];
                    if block_type(block_id) == 0 {
                        continue;
                    }

                    let tint = biome_tint_at(snapshot, chunk_x, chunk_z, x, z, biome_tints);
                    if is_custom_block(block_id) {
                        add_custom_block(
                            &mut batch,
                            snapshot,
                            texture_mapping,
                            biome_tints,
                            chunk_x,
                            chunk_z,
                            x,
                            base_y + y,
                            z,
                            block_id,
                            tint,
                            barrier_billboard,
                            voxel_ao_enabled,
                            voxel_ao_strength,
                            voxel_ao_cutout,
                        );
                        continue;
                    }

                    add_block_faces(
                        &mut batch,
                        snapshot,
                        texture_mapping,
                        biome_tints,
                        leaf_depth_layer_faces,
                        voxel_ao_enabled,
                        voxel_ao_strength,
                        voxel_ao_cutout,
                        vanilla_bake,
                        chunk_x,
                        chunk_z,
                        x,
                        base_y + y,
                        z,
                        block_id,
                        tint,
                    );
                }
            }
        }
    }

    batch
}

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
struct GreedyKey {
    texture_index: u16,
    block_id: u16,
    tint_key: u8,
    shade_key: u16,
}

#[derive(Debug)]
pub(super) struct GreedyQuad {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

pub(super) fn build_chunk_mesh_greedy(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    leaf_depth_layer_faces: bool,
    voxel_ao_enabled: bool,
    voxel_ao_strength: f32,
    voxel_ao_cutout: bool,
    barrier_billboard: bool,
    vanilla_bake: VanillaBakeSettings,
    texture_mapping: &AtlasBlockMapping,
    biome_tints: &BiomeTintResolver,
) -> MeshBatch {
    let mut batch = MeshBatch::default();

    let Some(column) = snapshot.columns.get(&(chunk_x, chunk_z)) else {
        return batch;
    };

    for (section_y, section_opt) in column.sections.iter().enumerate() {
        let Some(section_blocks) = section_opt else {
            continue;
        };
        let base_y = section_y as i32 * SECTION_HEIGHT;

        for y in 0..SECTION_HEIGHT {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let idx = (y * CHUNK_SIZE * CHUNK_SIZE + z * CHUNK_SIZE + x) as usize;
                    let block_id = section_blocks[idx];
                    if block_type(block_id) == 0 || !is_custom_block(block_id) {
                        continue;
                    }
                    let tint = biome_tint_at(snapshot, chunk_x, chunk_z, x, z, biome_tints);
                    add_custom_block(
                        &mut batch,
                        snapshot,
                        texture_mapping,
                        biome_tints,
                        chunk_x,
                        chunk_z,
                        x,
                        base_y + y,
                        z,
                        block_id,
                        tint,
                        barrier_billboard,
                        voxel_ao_enabled,
                        voxel_ao_strength,
                        voxel_ao_cutout,
                    );
                }
            }
        }

        for face in [
            Face::PosX,
            Face::NegX,
            Face::PosY,
            Face::NegY,
            Face::PosZ,
            Face::NegZ,
        ] {
            let mut planes = vec![HashMap::<GreedyKey, [u32; 16]>::new(); 16];

            for y in 0..SECTION_HEIGHT {
                for z in 0..CHUNK_SIZE {
                    for x in 0..CHUNK_SIZE {
                        let idx = (y * CHUNK_SIZE * CHUNK_SIZE + z * CHUNK_SIZE + x) as usize;
                        let block_id = section_blocks[idx];
                        if block_type(block_id) == 0 || is_custom_block(block_id) {
                            continue;
                        }

                        let (dx, dy, dz) = match face {
                            Face::PosX => (1, 0, 0),
                            Face::NegX => (-1, 0, 0),
                            Face::PosY => (0, 1, 0),
                            Face::NegY => (0, -1, 0),
                            Face::PosZ => (0, 0, 1),
                            Face::NegZ => (0, 0, -1),
                        };
                        let neighbor =
                            block_at(snapshot, chunk_x, chunk_z, x + dx, base_y + y + dy, z + dz);
                        if face_is_occluded(block_id, neighbor, leaf_depth_layer_faces) {
                            continue;
                        }

                        let texture_index = texture_mapping.texture_index_for_state(block_id, face);
                        let biome_id = biome_at(snapshot, chunk_x, chunk_z, x, z);
                        let tint_key = if !matches!(
                            classify_tint(block_id, None),
                            TintClass::None | TintClass::FoliageFixed(_)
                        ) {
                            biome_id
                        } else {
                            0
                        };
                        let key = GreedyKey {
                            texture_index,
                            block_id,
                            tint_key,
                            shade_key: face_shade_signature(
                                snapshot,
                                chunk_x,
                                chunk_z,
                                x,
                                base_y + y,
                                z,
                                face,
                                block_id,
                                voxel_ao_enabled,
                                voxel_ao_strength,
                                voxel_ao_cutout,
                                vanilla_bake,
                            ),
                        };

                        let (axis, u, v) = match face {
                            Face::PosY | Face::NegY => (y, x, z),
                            Face::PosX | Face::NegX => (x, z, y),
                            Face::PosZ | Face::NegZ => (z, x, y),
                        };
                        let entry = planes[axis as usize].entry(key).or_insert([0u32; 16]);
                        entry[u as usize] |= 1u32 << v;
                    }
                }
            }

            for (axis, plane) in planes.into_iter().enumerate() {
                for (key, data) in plane {
                    let quads = greedy_mesh_binary_plane(data, 16);
                    for quad in quads {
                        let tint = biome_tints.tint_for_biome(key.tint_key);
                        add_greedy_quad(
                            &mut batch,
                            snapshot,
                            chunk_x,
                            chunk_z,
                            face,
                            axis as i32,
                            base_y,
                            quad,
                            key.texture_index,
                            key.block_id,
                            tint,
                            voxel_ao_enabled,
                            voxel_ao_strength,
                            voxel_ao_cutout,
                            vanilla_bake,
                        );
                    }
                }
            }
        }
    }

    batch
}

#[allow(clippy::too_many_arguments)]
fn add_greedy_quad(
    batch: &mut MeshBatch,
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    face: Face,
    axis: i32,
    base_y: i32,
    quad: GreedyQuad,
    texture_index: u16,
    block_id: u16,
    tint: BiomeTint,
    voxel_ao_enabled: bool,
    voxel_ao_strength: f32,
    voxel_ao_cutout: bool,
    vanilla_bake: VanillaBakeSettings,
) {
    let data = batch.data_for(block_id);
    let base_index = data.positions.len() as u32;
    let tile_origin = atlas_tile_origin(texture_index);

    let u0 = quad.x as f32;
    let v0 = quad.y as f32;
    let u1 = u0 + quad.w as f32;
    let v1 = v0 + quad.h as f32;

    let (normal, verts) = match face {
        Face::PosY => {
            let y = (base_y + axis + 1) as f32;
            (
                [0.0, 1.0, 0.0],
                [[u0, y, v0], [u1, y, v0], [u1, y, v1], [u0, y, v1]],
            )
        }
        Face::NegY => {
            let y = (base_y + axis) as f32;
            (
                [0.0, -1.0, 0.0],
                [[u0, y, v1], [u1, y, v1], [u1, y, v0], [u0, y, v0]],
            )
        }
        Face::PosX => {
            let x = (axis + 1) as f32;
            let y0 = (base_y as f32) + v0;
            let y1 = (base_y as f32) + v1;
            (
                [1.0, 0.0, 0.0],
                [[x, y0, u0], [x, y0, u1], [x, y1, u1], [x, y1, u0]],
            )
        }
        Face::NegX => {
            let x = axis as f32;
            let y0 = (base_y as f32) + v0;
            let y1 = (base_y as f32) + v1;
            (
                [-1.0, 0.0, 0.0],
                [[x, y0, u1], [x, y0, u0], [x, y1, u0], [x, y1, u1]],
            )
        }
        Face::PosZ => {
            let z = (axis + 1) as f32;
            let y0 = (base_y as f32) + v0;
            let y1 = (base_y as f32) + v1;
            (
                [0.0, 0.0, 1.0],
                [[u1, y0, z], [u0, y0, z], [u0, y1, z], [u1, y1, z]],
            )
        }
        Face::NegZ => {
            let z = axis as f32;
            let y0 = (base_y as f32) + v0;
            let y1 = (base_y as f32) + v1;
            (
                [0.0, 0.0, -1.0],
                [[u0, y0, z], [u1, y0, z], [u1, y1, z], [u0, y1, z]],
            )
        }
    };

    for vert in verts {
        data.push_pos(vert);
        data.normals.push(normal);
    }

    let base_uvs = uv_for_texture();
    for uv in base_uvs {
        data.uvs
            .push([uv[0] * quad.w as f32, uv[1] * quad.h as f32]);
        data.uvs_b.push(tile_origin);
    }
    let base_color = tint_color_untargeted(block_id, tint, Some(vanilla_bake));
    let shades = greedy_face_corner_shades(
        snapshot,
        chunk_x,
        chunk_z,
        face,
        axis,
        base_y,
        &quad,
        block_id,
        voxel_ao_enabled,
        voxel_ao_strength,
        voxel_ao_cutout,
        vanilla_bake,
    );
    for shade in shades {
        if is_grass_side_face(block_id, face) {
            data.colors
                .push([base_color[0], base_color[1], base_color[2], shade]);
        } else {
            data.colors.push([
                base_color[0] * shade,
                base_color[1] * shade,
                base_color[2] * shade,
                base_color[3],
            ]);
        }
    }

    let use_alt_diag = (shades[0] + shades[2]) > (shades[1] + shades[3]);
    if use_alt_diag {
        data.indices.extend_from_slice(&[
            base_index,
            base_index + 3,
            base_index + 1,
            base_index + 1,
            base_index + 3,
            base_index + 2,
        ]);
    } else {
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

fn greedy_mesh_binary_plane(mut data: [u32; 16], size: u32) -> Vec<GreedyQuad> {
    let mut greedy_quads = Vec::new();
    for row in 0..data.len() {
        let mut y = 0;
        while y < size {
            y += (data[row] >> y).trailing_zeros();
            if y >= size {
                continue;
            }
            let h = (data[row] >> y).trailing_ones();
            let h_as_mask = u32::checked_shl(1, h).map_or(!0, |v| v - 1);
            let mask = h_as_mask << y;

            let mut w = 1;
            while row + w < size as usize {
                let next_row_h = (data[row + w] >> y) & h_as_mask;
                if next_row_h != h_as_mask {
                    break;
                }
                data[row + w] &= !mask;
                w += 1;
            }

            greedy_quads.push(GreedyQuad {
                y,
                w: w as u32,
                h,
                x: row as u32,
            });
            y += h;
        }
    }
    greedy_quads
}

#[allow(clippy::too_many_arguments)]
fn add_block_faces(
    batch: &mut MeshBatch,
    snapshot: &ChunkColumnSnapshot,
    texture_mapping: &AtlasBlockMapping,
    biome_tints: &BiomeTintResolver,
    leaf_depth_layer_faces: bool,
    voxel_ao_enabled: bool,
    voxel_ao_strength: f32,
    voxel_ao_cutout: bool,
    vanilla_bake: VanillaBakeSettings,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
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
                [1.0, 0.0, 0.0],
                [1.0, 0.0, 1.0],
                [1.0, 1.0, 1.0],
                [1.0, 1.0, 0.0],
            ],
        ),
        (
            Face::NegX,
            -1,
            0,
            0,
            [-1.0, 0.0, 0.0],
            [
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 1.0],
            ],
        ),
        (
            Face::PosY,
            0,
            1,
            0,
            [0.0, 1.0, 0.0],
            [
                [0.0, 1.0, 0.0],
                [1.0, 1.0, 0.0],
                [1.0, 1.0, 1.0],
                [0.0, 1.0, 1.0],
            ],
        ),
        (
            Face::NegY,
            0,
            -1,
            0,
            [0.0, -1.0, 0.0],
            [
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 1.0],
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
            ],
        ),
        (
            Face::PosZ,
            0,
            0,
            1,
            [0.0, 0.0, 1.0],
            [
                [1.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
                [0.0, 1.0, 1.0],
                [1.0, 1.0, 1.0],
            ],
        ),
        (
            Face::NegZ,
            0,
            0,
            -1,
            [0.0, 0.0, -1.0],
            [
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
        ),
    ];

    for (face, dx, dy, dz, normal, verts) in faces {
        let neighbor = block_at(snapshot, chunk_x, chunk_z, x + dx, y + dy, z + dz);
        if face_is_occluded(block_id, neighbor, leaf_depth_layer_faces) {
            continue;
        }

        let texture_index = texture_mapping.texture_index_for_state(block_id, face);
        let data = batch.data_for(block_id);
        let base_index = data.positions.len() as u32;
        for vert in verts {
            data.push_pos([vert[0] + x as f32, vert[1] + y as f32, vert[2] + z as f32]);
            data.normals.push(normal);
        }
        let uvs = uv_for_texture();
        data.uvs.extend_from_slice(&uvs);
        let tile_origin = atlas_tile_origin(texture_index);
        data.uvs_b.extend_from_slice(&[tile_origin; 4]);
        let base_color = tint_color(
            block_id,
            tint,
            snapshot,
            chunk_x,
            chunk_z,
            x,
            y,
            z,
            biome_tints,
            Some(vanilla_bake),
        );
        for vert in verts {
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
                vanilla_bake,
            );
            if is_grass_side_face(block_id, face) {
                data.colors
                    .push([base_color[0], base_color[1], base_color[2], shade]);
            } else {
                data.colors.push([
                    base_color[0] * shade,
                    base_color[1] * shade,
                    base_color[2] * shade,
                    base_color[3],
                ]);
            }
        }
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

pub(super) fn is_grass_side_face(block_state: u16, face: Face) -> bool {
    block_type(block_state) == 2 && !matches!(face, Face::PosY | Face::NegY)
}
