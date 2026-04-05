use bevy::prelude::*;

use crate::block_models::IconQuad;
use crate::block_textures::{AtlasBlockMapping, atlas_tile_origin};
use crate::chunk::{MeshData, build_mesh_from_data};
use crate::{BlockModelResolver, ModelFace};

fn atlas_texture_name(texture_path: &str) -> &str {
    texture_path
        .rsplit('/')
        .next()
        .unwrap_or(texture_path)
}

fn face_vertices(from: [f32; 3], to: [f32; 3], dir: &str) -> Option<[[f32; 3]; 4]> {
    let (x0, y0, z0) = (from[0] / 16.0, from[1] / 16.0, from[2] / 16.0);
    let (x1, y1, z1) = (to[0] / 16.0, to[1] / 16.0, to[2] / 16.0);
    let verts = match dir {
        "up" => [[x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]],
        "down" => [[x0, y0, z1], [x1, y0, z1], [x1, y0, z0], [x0, y0, z0]],
        "north" => [[x1, y0, z0], [x0, y0, z0], [x0, y1, z0], [x1, y1, z0]],
        "south" => [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
        "west" => [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]],
        "east" => [[x1, y0, z1], [x1, y0, z0], [x1, y1, z0], [x1, y1, z1]],
        _ => return None,
    };
    Some(verts)
}

fn rotate_uvs(uv: &mut [[f32; 2]; 4], rotation: i32) {
    let turns = ((rotation / 90) % 4 + 4) % 4;
    for _ in 0..turns {
        let old = *uv;
        uv[0] = old[3];
        uv[1] = old[0];
        uv[2] = old[1];
        uv[3] = old[2];
    }
}

fn uv_rect(u0: f32, v0: f32, u1: f32, v1: f32, rotation: i32) -> [[f32; 2]; 4] {
    let mut uv = [
        [u0 / 16.0, v0 / 16.0],
        [u1 / 16.0, v0 / 16.0],
        [u1 / 16.0, v1 / 16.0],
        [u0 / 16.0, v1 / 16.0],
    ];
    rotate_uvs(&mut uv, rotation);
    uv
}

fn rotate_y(vertices: [[f32; 3]; 4]) -> [[f32; 3]; 4] {
    vertices.map(|[x, y, z]| [z, y, 1.0 - x])
}

fn push_model_face(
    out: &mut Vec<IconQuad>,
    from: [f32; 3],
    to: [f32; 3],
    dir: &'static str,
    uv: [f32; 4],
    rotation: i32,
    texture_path: &str,
    rotate_x_aligned: bool,
) {
    let Some(mut vertices) = face_vertices(from, to, dir) else {
        return;
    };
    if rotate_x_aligned {
        vertices = rotate_y(vertices);
    }
    out.push(IconQuad {
        vertices,
        uv: uv_rect(uv[0], uv[1], uv[2], uv[3], rotation),
        texture_path: texture_path.to_string(),
        tint_index: None,
    });
}

fn anvil_top_texture(meta: u8) -> &'static str {
    match (meta >> 2).min(2) {
        1 => "blocks/anvil_top_damaged_1.png",
        2 => "blocks/anvil_top_damaged_2.png",
        _ => "blocks/anvil_top_damaged_0.png",
    }
}

pub fn anvil_display_quads(meta: u8, x_aligned: bool) -> Vec<IconQuad> {
    let body = "blocks/anvil_base.png";
    let top = anvil_top_texture(meta);
    let mut out = Vec::new();
    let elements = [
        (
            [2.0, 0.0, 2.0],
            [14.0, 4.0, 14.0],
            [
                ("down", [2.0, 2.0, 14.0, 14.0], 180, body),
                ("up", [2.0, 2.0, 14.0, 14.0], 180, body),
                ("north", [2.0, 12.0, 14.0, 16.0], 0, body),
                ("south", [2.0, 12.0, 14.0, 16.0], 0, body),
                ("west", [0.0, 2.0, 4.0, 14.0], 90, body),
                ("east", [4.0, 2.0, 0.0, 14.0], 270, body),
            ],
        ),
        (
            [4.0, 4.0, 3.0],
            [12.0, 5.0, 13.0],
            [
                ("down", [4.0, 3.0, 12.0, 13.0], 180, body),
                ("up", [4.0, 3.0, 12.0, 13.0], 180, body),
                ("north", [4.0, 11.0, 12.0, 12.0], 0, body),
                ("south", [4.0, 11.0, 12.0, 12.0], 0, body),
                ("west", [4.0, 3.0, 5.0, 13.0], 90, body),
                ("east", [5.0, 3.0, 4.0, 13.0], 270, body),
            ],
        ),
        (
            [6.0, 5.0, 4.0],
            [10.0, 10.0, 12.0],
            [
                ("down", [10.0, 12.0, 6.0, 4.0], 180, body),
                ("up", [10.0, 12.0, 6.0, 4.0], 180, body),
                ("north", [6.0, 6.0, 10.0, 11.0], 0, body),
                ("south", [6.0, 6.0, 10.0, 11.0], 0, body),
                ("west", [5.0, 4.0, 10.0, 12.0], 90, body),
                ("east", [10.0, 4.0, 5.0, 12.0], 270, body),
            ],
        ),
        (
            [3.0, 10.0, 0.0],
            [13.0, 16.0, 16.0],
            [
                ("down", [3.0, 0.0, 13.0, 16.0], 180, body),
                ("up", [3.0, 0.0, 13.0, 16.0], 180, top),
                ("north", [3.0, 0.0, 13.0, 6.0], 0, body),
                ("south", [3.0, 0.0, 13.0, 6.0], 0, body),
                ("west", [10.0, 0.0, 16.0, 16.0], 90, body),
                ("east", [16.0, 0.0, 10.0, 16.0], 270, body),
            ],
        ),
    ];

    for (from, to, faces) in elements {
        for (dir, uv, rotation, texture) in faces {
            push_model_face(&mut out, from, to, dir, uv, rotation, texture, x_aligned);
        }
    }
    out
}

#[derive(Clone, Copy)]
enum ChestFacing {
    South,
}

fn quad_uvs(u1: f32, v1: f32, u2: f32, v2: f32, tex_w: f32, tex_h: f32) -> [[f32; 2]; 4] {
    [
        [u2 / tex_w, v1 / tex_h],
        [u1 / tex_w, v1 / tex_h],
        [u1 / tex_w, v2 / tex_h],
        [u2 / tex_w, v2 / tex_h],
    ]
}

fn chest_model_to_local(vertex: [f32; 3], _facing: ChestFacing, _span_x: f32) -> [f32; 3] {
    [vertex[0] / 16.0, 1.0 - vertex[1] / 16.0, 1.0 - vertex[2] / 16.0]
}

fn push_chest_box_quads(
    out: &mut Vec<IconQuad>,
    texture_name: &str,
    texture_size: (f32, f32),
    texture_offset: (f32, f32),
    box_origin: [f32; 3],
    box_size: [f32; 3],
    render_bottom: bool,
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
        out.push(IconQuad {
            vertices: flipped_vertices
                .map(|vertex| chest_model_to_local(vertex, ChestFacing::South, 1.0)),
            uv: flipped_uv,
            texture_path: texture_name.to_string(),
            tint_index: None,
        });
    }
}

pub fn chest_display_quads(block_id: u16) -> Vec<IconQuad> {
    let texture_name = match block_id {
        130 => "entity/chest/ender.png",
        146 => "entity/chest/trapped.png",
        _ => "entity/chest/normal.png",
    };
    let mut out = Vec::new();
    push_chest_box_quads(
        &mut out,
        texture_name,
        (64.0, 64.0),
        (0.0, 19.0),
        [1.0, 6.0, 1.0],
        [14.0, 10.0, 14.0],
        false,
    );
    push_chest_box_quads(
        &mut out,
        texture_name,
        (64.0, 64.0),
        (0.0, 0.0),
        [1.0, 2.0, 1.0],
        [14.0, 5.0, 14.0],
        true,
    );
    push_chest_box_quads(
        &mut out,
        texture_name,
        (64.0, 64.0),
        (0.0, 0.0),
        [7.0, 5.0, 0.0],
        [2.0, 4.0, 1.0],
        true,
    );
    out
}

pub fn block_item_display_quads(
    block_id: u16,
    meta: u8,
    resolver: &mut BlockModelResolver,
) -> Option<Vec<IconQuad>> {
    match block_id {
        54 | 130 | 146 => return Some(chest_display_quads(block_id)),
        145 => return Some(anvil_display_quads(meta, matches!(meta & 0x3, 1 | 3))),
        _ => {}
    }

    resolver
        .icon_quads_for_meta(block_id, meta)
        .filter(|quads| !quads.is_empty())
        .or_else(|| resolver.block_item_icon_quads(block_id, meta))
}

fn face_shade(normal: Vec3) -> f32 {
    if normal.y.abs() > 0.8 {
        return 1.0;
    }
    if normal.x > 0.35 {
        return 0.82;
    }
    if normal.z > 0.35 {
        return 0.66;
    }
    if normal.x < -0.35 || normal.z < -0.35 {
        return 0.58;
    }
    0.72
}

pub fn build_block_display_mesh(
    quads: &[IconQuad],
    texture_mapping: &AtlasBlockMapping,
) -> (Mesh, Option<(Vec3, Vec3)>) {
    let mut data = MeshData::empty();
    for quad in quads {
        let tex_name = atlas_texture_name(&quad.texture_path);
        let texture_index = texture_mapping
            .texture_index_by_name(tex_name)
            .unwrap_or(texture_mapping.missing_index);
        let tile_origin = atlas_tile_origin(texture_index);

        let centered = quad.vertices.map(|[x, y, z]| [x - 0.5, y - 0.5, z - 0.5]);
        let v0 = Vec3::from_array(centered[0]);
        let v1 = Vec3::from_array(centered[1]);
        let v3 = Vec3::from_array(centered[3]);
        let normal = (v1 - v0).cross(v3 - v0).normalize_or_zero();
        let shade = face_shade(normal);
        let base_index = data.positions.len() as u32;
        for (i, vertex) in centered.iter().enumerate() {
            data.push_pos(*vertex);
            data.normals.push([normal.x, normal.y, normal.z]);
            data.uvs.push(quad.uv[i]);
            data.uvs_b.push(tile_origin);
            data.colors.push([shade, shade, shade, 1.0]);
        }
        data.indices.extend_from_slice(&[
            base_index,
            base_index + 1,
            base_index + 2,
            base_index,
            base_index + 2,
            base_index + 3,
        ]);
    }
    build_mesh_from_data(data)
}

pub fn face_texture_name_for_display_fallback(
    resolver: &mut BlockModelResolver,
    block_id: u16,
    meta: u8,
    face: ModelFace,
) -> Option<String> {
    resolver.face_texture_name_for_meta(block_id, meta, face)
}
