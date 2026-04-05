use super::*;
use crate::{
    IconQuad, anvil_display_quads, brewing_stand_display_quads, skull_display_quads,
    torch_display_quads,
};

fn atlas_texture_name(texture_path: &str) -> &str {
    texture_path
        .rsplit('/')
        .next()
        .unwrap_or(texture_path)
}

fn face_from_normal(normal: Vec3) -> Face {
    if normal.x.abs() >= normal.y.abs() && normal.x.abs() >= normal.z.abs() {
        if normal.x >= 0.0 {
            Face::PosX
        } else {
            Face::NegX
        }
    } else if normal.y.abs() >= normal.z.abs() {
        if normal.y >= 0.0 {
            Face::PosY
        } else {
            Face::NegY
        }
    } else if normal.z >= 0.0 {
        Face::PosZ
    } else {
        Face::NegZ
    }
}

#[derive(Clone, Copy)]
enum ChestFacing {
    North,
    South,
    West,
    East,
}

impl ChestFacing {
    fn from_meta(meta: u8) -> Self {
        match meta & 0x7 {
            2 => Self::North,
            4 => Self::West,
            5 => Self::East,
            _ => Self::South,
        }
    }
}

fn quad_uvs(u1: f32, v1: f32, u2: f32, v2: f32, tex_w: f32, tex_h: f32) -> [[f32; 2]; 4] {
    [
        [u2 / tex_w, v1 / tex_h],
        [u1 / tex_w, v1 / tex_h],
        [u1 / tex_w, v2 / tex_h],
        [u2 / tex_w, v2 / tex_h],
    ]
}

fn rotate_chest_local(vertex: [f32; 3], facing: ChestFacing, span_x: f32, span_z: f32) -> [f32; 3] {
    let center_x = span_x * 0.5;
    let center_z = span_z * 0.5;
    let dx = vertex[0] - center_x;
    let dz = vertex[2] - center_z;

    let (rx, rz) = match facing {
        ChestFacing::South => (dx, dz),
        ChestFacing::North => (-dx, -dz),
        ChestFacing::West => (-dz, dx),
        ChestFacing::East => (dz, -dx),
    };

    let corners = [
        [0.0, 0.0],
        [span_x, 0.0],
        [0.0, span_z],
        [span_x, span_z],
    ];
    let mut min_x = f32::INFINITY;
    let mut min_z = f32::INFINITY;
    for corner in corners {
        let cdx = corner[0] - center_x;
        let cdz = corner[1] - center_z;
        let (crx, crz) = match facing {
            ChestFacing::South => (cdx, cdz),
            ChestFacing::North => (-cdx, -cdz),
            ChestFacing::West => (-cdz, cdx),
            ChestFacing::East => (cdz, -cdx),
        };
        min_x = min_x.min(center_x + crx);
        min_z = min_z.min(center_z + crz);
    }

    [center_x + rx - min_x, vertex[1], center_z + rz - min_z]
}

fn chest_model_to_local(vertex: [f32; 3], facing: ChestFacing, span_x: f32) -> [f32; 3] {
    let canonical = [vertex[0] / 16.0, 1.0 - vertex[1] / 16.0, 1.0 - vertex[2] / 16.0];
    rotate_chest_local(canonical, facing, span_x, 1.0)
}

fn rotate_lid_vertex(vertex: [f32; 3], angle: f32) -> [f32; 3] {
    let pivot_y = 7.0;
    let pivot_z = 15.0;
    let dy = vertex[1] - pivot_y;
    let dz = vertex[2] - pivot_z;
    let sin = angle.sin();
    let cos = angle.cos();
    [
        vertex[0],
        pivot_y + dy * cos - dz * sin,
        pivot_z + dy * sin + dz * cos,
    ]
}

fn push_chest_box_quads(
    out: &mut Vec<IconQuad>,
    texture_name: &str,
    texture_size: (f32, f32),
    texture_offset: (f32, f32),
    box_origin: [f32; 3],
    box_size: [f32; 3],
    facing: ChestFacing,
    span_x: f32,
    render_bottom: bool,
    lid_angle: Option<f32>,
) {
    let (u, v) = texture_offset;
    let (dx, dy, dz) = (box_size[0], box_size[1], box_size[2]);
    let tex_w = texture_size.0;
    let tex_h = texture_size.1;
    let x1 = box_origin[0];
    let y1 = box_origin[1];
    let z1 = box_origin[2];
    let x2 = x1 + dx;
    let y2 = y1 + dy;
    let z2 = z1 + dz;

    let faces = [
        (
            [[x2, y1, z2], [x2, y1, z1], [x2, y2, z1], [x2, y2, z2]],
            quad_uvs(u + dz + dx, v + dz, u + dz + dx + dz, v + dz + dy, tex_w, tex_h),
        ),
        (
            [[x1, y1, z1], [x1, y1, z2], [x1, y2, z2], [x1, y2, z1]],
            quad_uvs(u, v + dz, u + dz, v + dz + dy, tex_w, tex_h),
        ),
        (
            [[x2, y1, z2], [x1, y1, z2], [x1, y1, z1], [x2, y1, z1]],
            quad_uvs(u + dz, v, u + dz + dx, v + dz, tex_w, tex_h),
        ),
        (
            [[x2, y2, z1], [x1, y2, z1], [x1, y2, z2], [x2, y2, z2]],
            quad_uvs(u + dz + dx, v + dz, u + dz + dx + dx, v, tex_w, tex_h),
        ),
        (
            [[x2, y1, z1], [x1, y1, z1], [x1, y2, z1], [x2, y2, z1]],
            quad_uvs(u + dz, v + dz, u + dz + dx, v + dz + dy, tex_w, tex_h),
        ),
        (
            [[x1, y1, z2], [x2, y1, z2], [x2, y2, z2], [x1, y2, z2]],
            quad_uvs(
                u + dz + dx + dz,
                v + dz,
                u + dz + dx + dz + dx,
                v + dz + dy,
                tex_w,
                tex_h,
            ),
        ),
    ];

    for (face_index, (vertices, uv)) in faces.into_iter().enumerate() {
        if face_index == 2 && !render_bottom {
            continue;
        }
        let flipped_vertices = [vertices[0], vertices[3], vertices[2], vertices[1]];
        let flipped_uv = [uv[0], uv[3], uv[2], uv[1]];
        let flipped_vertices = if let Some(angle) = lid_angle {
            flipped_vertices.map(|vertex| rotate_lid_vertex(vertex, angle))
        } else {
            flipped_vertices
        };
        out.push(IconQuad {
            vertices: flipped_vertices.map(|vertex| chest_model_to_local(vertex, facing, span_x)),
            uv: flipped_uv,
            texture_path: texture_name.to_string(),
            tint_index: None,
        });
    }
}

fn chest_pair_extents(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    block_id: u16,
) -> Option<(bool, f32, &'static str, (f32, f32))> {
    let id = block_type(block_id);
    let same = |dx: i32, dz: i32| block_type(block_at(snapshot, chunk_x, chunk_z, x + dx, y, z + dz)) == id;

    if id == 130 {
        return Some((true, 1.0, "chest_ender.png", (64.0, 64.0)));
    }

    if same(-1, 0) || same(0, -1) {
        return None;
    }

    if same(1, 0) || same(0, 1) {
        let texture = if id == 146 {
            "chest_trapped_double.png"
        } else {
            "chest_normal_double.png"
        };
        Some((false, 2.0, texture, (128.0, 64.0)))
    } else {
        let texture = if id == 146 {
            "chest_trapped.png"
        } else {
            "chest_normal.png"
        };
        Some((true, 1.0, texture, (64.0, 64.0)))
    }
}

#[allow(clippy::too_many_arguments)]
fn add_anvil_block(
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
    vanilla_bake: VanillaBakeSettings,
) {
    let meta = block_meta(block_id);
    let quads = anvil_display_quads(meta, matches!(meta & 0x3, 1 | 3));
    add_model_quads(
        batch,
        snapshot,
        texture_mapping,
        biome_tints,
        chunk_x,
        chunk_z,
        x,
        y,
        z,
        block_id,
        tint,
        &quads,
        false,
        0.0,
        false,
        0.0,
        vanilla_bake,
    );
}

#[allow(clippy::too_many_arguments)]
fn add_chest_block(
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
    voxel_ao_foliage_boost: f32,
    vanilla_bake: VanillaBakeSettings,
) {
    let Some((_single, span_x, texture_name, texture_size)) =
        chest_pair_extents(snapshot, chunk_x, chunk_z, x, y, z, block_id)
    else {
        return;
    };
    let facing = ChestFacing::from_meta(block_meta(block_id));
    let mut quads = Vec::new();
    let below_occluding = is_occluding_block(block_at(snapshot, chunk_x, chunk_z, x, y - 1, z));
    let lid_progress = snapshot
        .chest_states
        .get(&IVec3::new(x, y, z))
        .map(|state| state.progress)
        .unwrap_or(0.0);
    let eased = 1.0 - (1.0 - lid_progress).powi(3);
    let lid_angle = -(eased * std::f32::consts::FRAC_PI_2);

    push_chest_box_quads(
        &mut quads,
        texture_name,
        texture_size,
        (0.0, 19.0),
        [1.0, 6.0, 1.0],
        [span_x * 16.0 - 2.0, 10.0, 14.0],
        facing,
        span_x,
        !below_occluding,
        None,
    );
    push_chest_box_quads(
        &mut quads,
        texture_name,
        texture_size,
        (0.0, 0.0),
        [1.0, 2.0, 1.0],
        [span_x * 16.0 - 2.0, 5.0, 14.0],
        facing,
        span_x,
        true,
        Some(lid_angle),
    );
    push_chest_box_quads(
        &mut quads,
        texture_name,
        texture_size,
        (0.0, 0.0),
        [span_x * 8.0 - 1.0, 5.0, 0.0],
        [2.0, 4.0, 1.0],
        facing,
        span_x,
        true,
        Some(lid_angle),
    );

    add_model_quads(
        batch,
        snapshot,
        texture_mapping,
        biome_tints,
        chunk_x,
        chunk_z,
        x,
        y,
        z,
        block_id,
        tint,
        &quads,
        voxel_ao_enabled,
        voxel_ao_strength,
        voxel_ao_cutout,
        voxel_ao_foliage_boost,
        vanilla_bake,
    );
}

#[allow(clippy::too_many_arguments)]
fn add_model_quads(
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
    quads: &[IconQuad],
    voxel_ao_enabled: bool,
    voxel_ao_strength: f32,
    voxel_ao_cutout: bool,
    voxel_ao_foliage_boost: f32,
    vanilla_bake: VanillaBakeSettings,
) {
    let data = batch.data_for(block_id);
    for quad in quads {
        let tex_name = atlas_texture_name(&quad.texture_path);
        let texture_index = texture_mapping
            .texture_index_by_name(tex_name)
            .unwrap_or(texture_mapping.missing_index);
        let tile_origin = atlas_tile_origin(texture_index);

        let v0 = Vec3::from_array(quad.vertices[0]);
        let v1 = Vec3::from_array(quad.vertices[1]);
        let v3 = Vec3::from_array(quad.vertices[3]);
        let normal = (v1 - v0).cross(v3 - v0).normalize_or_zero();
        let face = face_from_normal(normal);
        let base_color = if quad.tint_index.is_some() {
            tint_color(
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
            )
        } else {
            [1.0, 1.0, 1.0, 1.0]
        };

        let base_index = data.positions.len() as u32;
        for (i, vert) in quad.vertices.iter().enumerate() {
            data.push_pos([vert[0] + x as f32, vert[1] + y as f32, vert[2] + z as f32]);
            data.normals.push([normal.x, normal.y, normal.z]);
            data.uvs.push(quad.uv[i]);
            data.uvs_b.push(tile_origin);
            let shade = compute_vertex_shade(
                snapshot,
                chunk_x,
                chunk_z,
                x,
                y,
                z,
                face,
                *vert,
                block_id,
                voxel_ao_enabled,
                voxel_ao_strength,
                voxel_ao_cutout,
                voxel_ao_foliage_boost,
                vanilla_bake,
            );
            data.colors.push([
                base_color[0] * shade,
                base_color[1] * shade,
                base_color[2] * shade,
                base_color[3],
            ]);
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

#[allow(clippy::too_many_arguments)]
fn add_vine_block(
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
    voxel_ao_foliage_boost: f32,
    vanilla_bake: VanillaBakeSettings,
) {
    let meta = block_meta(block_id);
    let up = {
        let above = block_at(snapshot, chunk_x, chunk_z, x, y + 1, z);
        let above_id = block_type(above);
        above_id != 0 && !is_transparent_block(above_id) && block_model_kind(above_id) == BlockModelKind::FullCube
    };
    let mut quads = Vec::new();
    let add = |quads: &mut Vec<IconQuad>, vertices: [[f32; 3]; 4]| {
        quads.push(IconQuad {
            vertices,
            uv: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            texture_path: "blocks/vine.png".to_string(),
            tint_index: Some(0),
        });
    };

    if (meta & 0x4) != 0 {
        add(&mut quads, [[1.0, 0.0, 0.0625], [0.0, 0.0, 0.0625], [0.0, 1.0, 0.0625], [1.0, 1.0, 0.0625]]);
    }
    if (meta & 0x1) != 0 {
        add(&mut quads, [[0.0, 0.0, 0.9375], [1.0, 0.0, 0.9375], [1.0, 1.0, 0.9375], [0.0, 1.0, 0.9375]]);
    }
    if (meta & 0x2) != 0 {
        add(&mut quads, [[0.0625, 0.0, 0.0], [0.0625, 0.0, 1.0], [0.0625, 1.0, 1.0], [0.0625, 1.0, 0.0]]);
    }
    if (meta & 0x8) != 0 {
        add(&mut quads, [[0.9375, 0.0, 1.0], [0.9375, 0.0, 0.0], [0.9375, 1.0, 0.0], [0.9375, 1.0, 1.0]]);
    }
    if quads.is_empty() && up {
        add(&mut quads, [[0.0, 0.9375, 1.0], [1.0, 0.9375, 1.0], [1.0, 0.9375, 0.0], [0.0, 0.9375, 0.0]]);
    } else if up {
        add(&mut quads, [[0.0, 0.9375, 1.0], [1.0, 0.9375, 1.0], [1.0, 0.9375, 0.0], [0.0, 0.9375, 0.0]]);
    }

    add_model_quads(
        batch,
        snapshot,
        texture_mapping,
        biome_tints,
        chunk_x,
        chunk_z,
        x,
        y,
        z,
        block_id,
        tint,
        &quads,
        voxel_ao_enabled,
        voxel_ao_strength,
        voxel_ao_cutout,
        voxel_ao_foliage_boost,
        vanilla_bake,
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn add_named_custom_block(
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
    barrier_billboard: bool,
    voxel_ao_enabled: bool,
    voxel_ao_strength: f32,
    voxel_ao_cutout: bool,
    voxel_ao_foliage_boost: f32,
    vanilla_bake: VanillaBakeSettings,
) {
    let id = block_type(block_id);
    match id {
        106 => add_vine_block(
            batch,
            snapshot,
            texture_mapping,
            biome_tints,
            chunk_x,
            chunk_z,
            x,
            y,
            z,
            block_id,
            tint,
            voxel_ao_enabled,
            voxel_ao_strength,
            voxel_ao_cutout,
            voxel_ao_foliage_boost,
            vanilla_bake,
        ),
        145 => add_anvil_block(
            batch,
            snapshot,
            texture_mapping,
            biome_tints,
            chunk_x,
            chunk_z,
            x,
            y,
            z,
            block_id,
            tint,
            vanilla_bake,
        ),
        54 | 130 | 146 => add_chest_block(
            batch,
            snapshot,
            texture_mapping,
            biome_tints,
            chunk_x,
            chunk_z,
            x,
            y,
            z,
            block_id,
            tint,
            voxel_ao_enabled,
            voxel_ao_strength,
            voxel_ao_cutout,
            voxel_ao_foliage_boost,
            vanilla_bake,
        ),
        26 => add_box(
            batch,
            Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [0.0, 0.0, 0.0],
            [1.0, 9.0 / 16.0, 1.0],
            block_id,
            tint,
        ),
        60 => add_box(
            batch,
            Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [0.0, 0.0, 0.0],
            [1.0, 0.9375, 1.0],
            block_id,
            tint,
        ),
        27 | 28 | 66 | 157 => add_box(
            batch,
            Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [0.0, 0.0, 0.0],
            [1.0, 1.0 / 16.0, 1.0],
            block_id,
            tint,
        ),
        64 | 71 | 193 | 194 | 195 | 196 | 197 => {
            let meta = block_meta(block_id);
            let lower_meta = if (meta & 0x8) != 0 {
                let below = block_at(snapshot, chunk_x, chunk_z, x, y - 1, z);
                if block_type(below) == id {
                    block_meta(below)
                } else {
                    0
                }
            } else {
                meta
            };
            let facing = lower_meta & 0x3;
            let is_open = (lower_meta & 0x4) != 0;
            let t = 3.0 / 16.0;
            let (min, max) = if !is_open {
                match facing {
                    0 => ([0.0, 0.0, 0.0], [t, 1.0, 1.0]),
                    1 => ([0.0, 0.0, 0.0], [1.0, 1.0, t]),
                    2 => ([1.0 - t, 0.0, 0.0], [1.0, 1.0, 1.0]),
                    _ => ([0.0, 0.0, 1.0 - t], [1.0, 1.0, 1.0]),
                }
            } else {
                match facing {
                    0 => ([0.0, 0.0, 1.0 - t], [1.0, 1.0, 1.0]),
                    1 => ([0.0, 0.0, 0.0], [t, 1.0, 1.0]),
                    2 => ([0.0, 0.0, 0.0], [1.0, 1.0, t]),
                    _ => ([1.0 - t, 0.0, 0.0], [1.0, 1.0, 1.0]),
                }
            };
            add_box(
                batch,
                Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
                texture_mapping,
                biome_tints,
                x,
                y,
                z,
                min,
                max,
                block_id,
                tint,
            );
        }
        65 => {
            let t = 1.0 / 16.0;
            let (min, max) = match block_meta(block_id) & 0x7 {
                2 => ([0.0, 0.0, 1.0 - t], [1.0, 1.0, 1.0]),
                3 => ([0.0, 0.0, 0.0], [1.0, 1.0, t]),
                4 => ([1.0 - t, 0.0, 0.0], [1.0, 1.0, 1.0]),
                5 => ([0.0, 0.0, 0.0], [t, 1.0, 1.0]),
                _ => ([0.0, 0.0, 0.0], [1.0, 1.0, t]),
            };
            add_box(
                batch,
                Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
                texture_mapping,
                biome_tints,
                x,
                y,
                z,
                min,
                max,
                block_id,
                tint,
            );
        }
        107 | 183 | 184 | 185 | 186 | 187 => {
            let meta = block_meta(block_id);
            let facing = meta & 0x3;
            let is_open = (meta & 0x4) != 0;
            let x_aligned = matches!(facing, 0 | 2);
            let t = 0.125;
            let rail_min = 0.375;
            let rail_max = 0.625;

            let (panel_min, panel_max) = if !is_open {
                if x_aligned {
                    ([0.0, 0.0, rail_min], [1.0, 1.0, rail_max])
                } else {
                    ([rail_min, 0.0, 0.0], [rail_max, 1.0, 1.0])
                }
            } else if x_aligned {
                ([rail_min, 0.0, 0.0], [rail_max, 1.0, 1.0])
            } else {
                ([0.0, 0.0, rail_min], [1.0, 1.0, rail_max])
            };
            add_box(
                batch,
                Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
                texture_mapping,
                biome_tints,
                x,
                y,
                z,
                panel_min,
                panel_max,
                block_id,
                tint,
            );

            if x_aligned {
                add_box(
                    batch,
                    None,
                    texture_mapping,
                    biome_tints,
                    x,
                    y,
                    z,
                    [0.0, 0.0, 0.4375],
                    [t, 1.0, 0.5625],
                    block_id,
                    tint,
                );
                add_box(
                    batch,
                    None,
                    texture_mapping,
                    biome_tints,
                    x,
                    y,
                    z,
                    [1.0 - t, 0.0, 0.4375],
                    [1.0, 1.0, 0.5625],
                    block_id,
                    tint,
                );
            } else {
                add_box(
                    batch,
                    None,
                    texture_mapping,
                    biome_tints,
                    x,
                    y,
                    z,
                    [0.4375, 0.0, 0.0],
                    [0.5625, 1.0, t],
                    block_id,
                    tint,
                );
                add_box(
                    batch,
                    None,
                    texture_mapping,
                    biome_tints,
                    x,
                    y,
                    z,
                    [0.4375, 0.0, 1.0 - t],
                    [0.5625, 1.0, 1.0],
                    block_id,
                    tint,
                );
            }
        }
        139 => add_wall_block(
            batch,
            snapshot,
            texture_mapping,
            biome_tints,
            chunk_x,
            chunk_z,
            x,
            y,
            z,
            block_id,
            tint,
        ),
        78 => {
            let layers = (block_meta(block_id) & 0x7) + 1;
            let h = (layers as f32 / 8.0).clamp(0.125, 1.0);
            add_box(
                batch,
                Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
                texture_mapping,
                biome_tints,
                x,
                y,
                z,
                [0.0, 0.0, 0.0],
                [1.0, h, 1.0],
                block_id,
                tint,
            );
        }
        81 => add_box(
            batch,
            Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [1.0 / 16.0, 0.0, 1.0 / 16.0],
            [15.0 / 16.0, 1.0, 15.0 / 16.0],
            block_id,
            tint,
        ),
        88 => add_box(
            batch,
            Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [0.0, 0.0, 0.0],
            [1.0, 0.875, 1.0],
            block_id,
            tint,
        ),
        96 => {
            let meta = block_meta(block_id);
            let is_open = (meta & 0x4) != 0;
            let is_top = (meta & 0x8) != 0;
            let t = 3.0 / 16.0;
            let (min, max) = if is_open {
                match meta & 0x3 {
                    0 => ([0.0, 0.0, 1.0 - t], [1.0, 1.0, 1.0]),
                    1 => ([0.0, 0.0, 0.0], [1.0, 1.0, t]),
                    2 => ([1.0 - t, 0.0, 0.0], [1.0, 1.0, 1.0]),
                    _ => ([0.0, 0.0, 0.0], [t, 1.0, 1.0]),
                }
            } else if is_top {
                ([0.0, 1.0 - t, 0.0], [1.0, 1.0, 1.0])
            } else {
                ([0.0, 0.0, 0.0], [1.0, t, 1.0])
            };
            add_box(
                batch,
                Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
                texture_mapping,
                biome_tints,
                x,
                y,
                z,
                min,
                max,
                block_id,
                tint,
            );
        }
        171 => add_box(
            batch,
            Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [0.0, 0.0, 0.0],
            [1.0, 1.0 / 16.0, 1.0],
            block_id,
            tint,
        ),
        69 => {
            add_box(
                batch,
                None,
                texture_mapping,
                biome_tints,
                x,
                y,
                z,
                [0.3125, 0.0, 0.3125],
                [0.6875, 0.1875, 0.6875],
                block_id,
                tint,
            );
            add_box(
                batch,
                None,
                texture_mapping,
                biome_tints,
                x,
                y,
                z,
                [0.4375, 0.1875, 0.4375],
                [0.5625, 0.75, 0.5625],
                block_id,
                tint,
            );
        }
        63 => add_sign_post_block(batch, texture_mapping, biome_tints, x, y, z, block_id, tint),
        68 => add_wall_sign_block(batch, texture_mapping, biome_tints, x, y, z, block_id, tint),
        140 => add_box(
            batch,
            None,
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [0.3125, 0.0, 0.3125],
            [0.6875, 0.375, 0.6875],
            block_id,
            tint,
        ),
        50 | 75 | 76 => add_model_quads(
            batch,
            snapshot,
            texture_mapping,
            biome_tints,
            chunk_x,
            chunk_z,
            x,
            y,
            z,
            block_id,
            tint,
            &torch_display_quads(block_id, block_meta(block_id)),
            voxel_ao_enabled,
            voxel_ao_strength,
            voxel_ao_cutout,
            voxel_ao_foliage_boost,
            vanilla_bake,
        ),
        117 => add_model_quads(
            batch,
            snapshot,
            texture_mapping,
            biome_tints,
            chunk_x,
            chunk_z,
            x,
            y,
            z,
            block_id,
            tint,
            &brewing_stand_display_quads(block_meta(block_id)),
            voxel_ao_enabled,
            voxel_ao_strength,
            voxel_ao_cutout,
            voxel_ao_foliage_boost,
            vanilla_bake,
        ),
        144 => add_model_quads(
            batch,
            snapshot,
            texture_mapping,
            biome_tints,
            chunk_x,
            chunk_z,
            x,
            y,
            z,
            block_id,
            tint,
            &skull_display_quads(block_meta(block_id)),
            false,
            0.0,
            voxel_ao_cutout,
            voxel_ao_foliage_boost,
            vanilla_bake,
        ),
        166 => {
            if barrier_billboard {
                add_cross_plant(
                    batch,
                    snapshot,
                    texture_mapping,
                    biome_tints,
                    chunk_x,
                    chunk_z,
                    x,
                    y,
                    z,
                    block_id,
                    tint,
                    voxel_ao_enabled,
                    voxel_ao_strength,
                    voxel_ao_cutout,
                    voxel_ao_foliage_boost,
                );
            } else {
                add_box(
                    batch,
                    None,
                    texture_mapping,
                    biome_tints,
                    x,
                    y,
                    z,
                    [0.0, 0.0, 0.0],
                    [1.0, 1.0, 1.0],
                    block_id,
                    tint,
                );
            }
        }
        51 => add_cross_plant(
            batch,
            snapshot,
            texture_mapping,
            biome_tints,
            chunk_x,
            chunk_z,
            x,
            y,
            z,
            block_id,
            tint,
            voxel_ao_enabled,
            voxel_ao_strength,
            voxel_ao_cutout,
            voxel_ao_foliage_boost,
        ),
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn add_wall_block(
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
) {
    let connect_east = wall_connects_to(block_at(snapshot, chunk_x, chunk_z, x + 1, y, z));
    let connect_west = wall_connects_to(block_at(snapshot, chunk_x, chunk_z, x - 1, y, z));
    let connect_south = wall_connects_to(block_at(snapshot, chunk_x, chunk_z, x, y, z + 1));
    let connect_north = wall_connects_to(block_at(snapshot, chunk_x, chunk_z, x, y, z - 1));
    let has_x = connect_east || connect_west;
    let has_z = connect_north || connect_south;
    let center_tall = !has_x || !has_z;

    add_box(
        batch,
        None,
        texture_mapping,
        biome_tints,
        x,
        y,
        z,
        [0.25, 0.0, 0.25],
        [0.75, if center_tall { 1.0 } else { 0.8125 }, 0.75],
        block_id,
        tint,
    );
    if connect_north {
        add_box(
            batch,
            None,
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [0.3125, 0.0, 0.0],
            [0.6875, 0.8125, 0.5],
            block_id,
            tint,
        );
    }
    if connect_south {
        add_box(
            batch,
            None,
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [0.3125, 0.0, 0.5],
            [0.6875, 0.8125, 1.0],
            block_id,
            tint,
        );
    }
    if connect_west {
        add_box(
            batch,
            None,
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [0.0, 0.0, 0.3125],
            [0.5, 0.8125, 0.6875],
            block_id,
            tint,
        );
    }
    if connect_east {
        add_box(
            batch,
            None,
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            [0.5, 0.0, 0.3125],
            [1.0, 0.8125, 0.6875],
            block_id,
            tint,
        );
    }
}

fn add_sign_post_block(
    batch: &mut MeshBatch,
    texture_mapping: &AtlasBlockMapping,
    biome_tints: &BiomeTintResolver,
    x: i32,
    y: i32,
    z: i32,
    block_id: u16,
    tint: BiomeTint,
) {
    let yaw = (block_meta(block_id) as f32) * std::f32::consts::TAU / 16.0;
    let sx = yaw.sin().abs();
    let sz = yaw.cos().abs();
    let half_thickness = 0.0625 + 0.0625 * sx.max(sz);
    let min_x = (0.5 - half_thickness).clamp(0.0, 1.0);
    let max_x = (0.5 + half_thickness).clamp(0.0, 1.0);
    let min_z = (0.5 - half_thickness).clamp(0.0, 1.0);
    let max_z = (0.5 + half_thickness).clamp(0.0, 1.0);
    add_box(
        batch,
        None,
        texture_mapping,
        biome_tints,
        x,
        y,
        z,
        [min_x, 0.5, min_z],
        [max_x, 1.0, max_z],
        block_id,
        tint,
    );
    add_box(
        batch,
        None,
        texture_mapping,
        biome_tints,
        x,
        y,
        z,
        [0.4375, 0.0, 0.4375],
        [0.5625, 0.5, 0.5625],
        block_id,
        tint,
    );
}

fn add_wall_sign_block(
    batch: &mut MeshBatch,
    texture_mapping: &AtlasBlockMapping,
    biome_tints: &BiomeTintResolver,
    x: i32,
    y: i32,
    z: i32,
    block_id: u16,
    tint: BiomeTint,
) {
    let (min, max) = match block_meta(block_id) & 0x7 {
        2 => ([0.0, 0.25, 0.875], [1.0, 0.875, 0.9375]),
        3 => ([0.0, 0.25, 0.0625], [1.0, 0.875, 0.125]),
        4 => ([0.875, 0.25, 0.0], [0.9375, 0.875, 1.0]),
        _ => ([0.0625, 0.25, 0.0], [0.125, 0.875, 1.0]),
    };
    add_box(
        batch,
        None,
        texture_mapping,
        biome_tints,
        x,
        y,
        z,
        min,
        max,
        block_id,
        tint,
    );
}
