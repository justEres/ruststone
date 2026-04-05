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

fn flip_v(uv: [[f32; 2]; 4]) -> [[f32; 2]; 4] {
    uv.map(|[u, v]| [u, 1.0 - v])
}

fn rotate_y(vertices: [[f32; 3]; 4]) -> [[f32; 3]; 4] {
    vertices.map(|[x, y, z]| [z, y, 1.0 - x])
}

fn rotate_vertices_y(vertices: [[f32; 3]; 4], quarter_turns: i32) -> [[f32; 3]; 4] {
    let mut out = vertices;
    let turns = quarter_turns.rem_euclid(4);
    for _ in 0..turns {
        out = rotate_y(out);
    }
    out
}

fn rotate_vertices_around_z(
    vertices: [[f32; 3]; 4],
    origin: [f32; 3],
    angle_deg: f32,
) -> [[f32; 3]; 4] {
    let angle = angle_deg.to_radians();
    let sin = angle.sin();
    let cos = angle.cos();
    vertices.map(|[x, y, z]| {
        let dx = x - origin[0];
        let dy = y - origin[1];
        [
            origin[0] + dx * cos - dy * sin,
            origin[1] + dx * sin + dy * cos,
            z,
        ]
    })
}

fn rotate_vertices_around_y(
    vertices: [[f32; 3]; 4],
    origin: [f32; 3],
    angle_rad: f32,
) -> [[f32; 3]; 4] {
    let sin = angle_rad.sin();
    let cos = angle_rad.cos();
    vertices.map(|[x, y, z]| {
        let dx = x - origin[0];
        let dz = z - origin[2];
        [
            origin[0] + dx * cos + dz * sin,
            y,
            origin[2] - dx * sin + dz * cos,
        ]
    })
}

fn transform_vertices(
    vertices: [[f32; 3]; 4],
    scale: [f32; 3],
    translate: [f32; 3],
) -> [[f32; 3]; 4] {
    vertices.map(|[x, y, z]| {
        [
            x * scale[0] + translate[0],
            y * scale[1] + translate[1],
            z * scale[2] + translate[2],
        ]
    })
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

fn push_box_quads(
    out: &mut Vec<IconQuad>,
    texture_path: &str,
    texture_size: (f32, f32),
    texture_offset: (f32, f32),
    box_origin: [f32; 3],
    box_size: [f32; 3],
    hide_bottom: bool,
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
            false,
            [[x2, y1, z2], [x2, y1, z1], [x2, y2, z1], [x2, y2, z2]],
            quad_uvs(u + dz + dx, v + dz, u + dz + dx + dz, v + dz + dy, tex_w, tex_h),
        ),
        (
            false,
            [[x1, y1, z1], [x1, y1, z2], [x1, y2, z2], [x1, y2, z1]],
            quad_uvs(u, v + dz, u + dz, v + dz + dy, tex_w, tex_h),
        ),
        (
            hide_bottom,
            [[x2, y1, z2], [x1, y1, z2], [x1, y1, z1], [x2, y1, z1]],
            quad_uvs(u + dz, v, u + dz + dx, v + dz, tex_w, tex_h),
        ),
        (
            false,
            [[x2, y2, z1], [x1, y2, z1], [x1, y2, z2], [x2, y2, z2]],
            quad_uvs(u + dz + dx, v + dz, u + dz + dx + dx, v, tex_w, tex_h),
        ),
        (
            false,
            [[x2, y1, z1], [x1, y1, z1], [x1, y2, z1], [x2, y2, z1]],
            quad_uvs(u + dz, v + dz, u + dz + dx, v + dz + dy, tex_w, tex_h),
        ),
        (
            false,
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

    for (skip, vertices, uv) in faces {
        if skip {
            continue;
        }
        out.push(IconQuad {
            vertices: vertices.map(|[x, y, z]| [x / 16.0, y / 16.0, z / 16.0]),
            uv: flip_v(uv),
            texture_path: texture_path.to_string(),
            tint_index: None,
        });
    }
}

fn push_rotated_plane(
    out: &mut Vec<IconQuad>,
    texture_path: &str,
    from: [f32; 3],
    to: [f32; 3],
    quarter_turns: i32,
    uv_front: [f32; 4],
    uv_back: [f32; 4],
) {
    let north = face_vertices(from, to, "north").unwrap();
    let south = face_vertices(from, to, "south").unwrap();
    out.push(IconQuad {
        vertices: rotate_vertices_y(north, quarter_turns),
        uv: flip_v(uv_rect(uv_front[0], uv_front[1], uv_front[2], uv_front[3], 0)),
        texture_path: texture_path.to_string(),
        tint_index: None,
    });
    out.push(IconQuad {
        vertices: rotate_vertices_y(south, quarter_turns),
        uv: flip_v(uv_rect(uv_back[0], uv_back[1], uv_back[2], uv_back[3], 0)),
        texture_path: texture_path.to_string(),
        tint_index: None,
    });
}

pub fn torch_display_quads(block_id: u16, meta: u8) -> Vec<IconQuad> {
    let texture_path = match block_id {
        75 => "blocks/redstone_torch_off.png",
        76 => "blocks/redstone_torch_on.png",
        _ => "blocks/torch_on.png",
    };
    let mut out = Vec::new();
    if let Some(turns) = match meta & 0x7 {
        1 => Some(0),
        2 => Some(2),
        3 => Some(1),
        4 => Some(3),
        _ => None,
    } {
        for (dir, uv, from, to) in [
            ("down", [7.0, 13.0, 9.0, 15.0], [-1.0, 3.5, 7.0], [1.0, 13.5, 9.0]),
            ("up", [7.0, 6.0, 9.0, 8.0], [-1.0, 3.5, 7.0], [1.0, 13.5, 9.0]),
            ("west", [0.0, 0.0, 16.0, 16.0], [-1.0, 3.5, 0.0], [1.0, 19.5, 16.0]),
            ("east", [0.0, 0.0, 16.0, 16.0], [-1.0, 3.5, 0.0], [1.0, 19.5, 16.0]),
            ("north", [0.0, 0.0, 16.0, 16.0], [-8.0, 3.5, 7.0], [8.0, 19.5, 9.0]),
            ("south", [0.0, 0.0, 16.0, 16.0], [-8.0, 3.5, 7.0], [8.0, 19.5, 9.0]),
        ] {
            let verts = face_vertices(from, to, dir)
                .unwrap()
                .map(|[x, y, z]| [x / 16.0, y / 16.0, z / 16.0]);
            out.push(IconQuad {
                vertices: rotate_vertices_y(
                    rotate_vertices_around_z(verts, [0.0, 3.5 / 16.0, 0.5], -22.5),
                    turns,
                ),
                uv: flip_v(uv_rect(uv[0], uv[1], uv[2], uv[3], 0)),
                texture_path: texture_path.to_string(),
                tint_index: None,
            });
        }
    } else {
        for (dir, uv, from, to) in [
            ("down", [7.0, 13.0, 9.0, 15.0], [7.0, 0.0, 7.0], [9.0, 10.0, 9.0]),
            ("up", [7.0, 6.0, 9.0, 8.0], [7.0, 0.0, 7.0], [9.0, 10.0, 9.0]),
            ("west", [0.0, 0.0, 16.0, 16.0], [7.0, 0.0, 0.0], [9.0, 16.0, 16.0]),
            ("east", [0.0, 0.0, 16.0, 16.0], [7.0, 0.0, 0.0], [9.0, 16.0, 16.0]),
            ("north", [0.0, 0.0, 16.0, 16.0], [0.0, 0.0, 7.0], [16.0, 16.0, 9.0]),
            ("south", [0.0, 0.0, 16.0, 16.0], [0.0, 0.0, 7.0], [16.0, 16.0, 9.0]),
        ] {
            out.push(IconQuad {
                vertices: face_vertices(from, to, dir).unwrap(),
                uv: flip_v(uv_rect(uv[0], uv[1], uv[2], uv[3], 0)),
                texture_path: texture_path.to_string(),
                tint_index: None,
            });
        }
    }
    out
}

pub fn brewing_stand_display_quads(meta: u8) -> Vec<IconQuad> {
    let mut out = Vec::new();
    push_box_quads(
        &mut out,
        "blocks/brewing_stand.png",
        (16.0, 16.0),
        (7.0, 2.0),
        [7.0, 0.0, 7.0],
        [2.0, 14.0, 2.0],
        false,
    );
    push_box_quads(
        &mut out,
        "blocks/brewing_stand_base.png",
        (16.0, 16.0),
        (9.0, 5.0),
        [9.0, 0.0, 5.0],
        [6.0, 2.0, 6.0],
        false,
    );
    push_box_quads(
        &mut out,
        "blocks/brewing_stand_base.png",
        (16.0, 16.0),
        (2.0, 1.0),
        [2.0, 0.0, 1.0],
        [6.0, 2.0, 6.0],
        false,
    );
    push_box_quads(
        &mut out,
        "blocks/brewing_stand_base.png",
        (16.0, 16.0),
        (2.0, 9.0),
        [2.0, 0.0, 9.0],
        [6.0, 2.0, 6.0],
        false,
    );

    push_rotated_plane(
        &mut out,
        "blocks/brewing_stand.png",
        [8.0, 0.0, 8.0],
        [16.0, 16.0, 8.0],
        0,
        [0.0, 0.0, 8.0, 16.0],
        [8.0, 0.0, 0.0, 16.0],
    );
    for angle in [45.0f32, -45.0f32] {
        let north = face_vertices([0.0, 0.0, 8.0], [8.0, 16.0, 8.0], "north").unwrap();
        let south = face_vertices([0.0, 0.0, 8.0], [8.0, 16.0, 8.0], "south").unwrap();
        out.push(IconQuad {
            vertices: rotate_vertices_around_z(rotate_vertices_y(north, 1), [0.5, 0.5, 0.5], 0.0)
                .map(|v| {
                    let dx = v[0] - 0.5;
                    let dz = v[2] - 0.5;
                    let r = angle.to_radians();
                    [0.5 + dx * r.cos() - dz * r.sin(), v[1], 0.5 + dx * r.sin() + dz * r.cos()]
                }),
            uv: flip_v(uv_rect(8.0, 0.0, 0.0, 16.0, 0)),
            texture_path: "blocks/brewing_stand.png".to_string(),
            tint_index: None,
        });
        out.push(IconQuad {
            vertices: rotate_vertices_around_z(rotate_vertices_y(south, 1), [0.5, 0.5, 0.5], 0.0)
                .map(|v| {
                    let dx = v[0] - 0.5;
                    let dz = v[2] - 0.5;
                    let r = angle.to_radians();
                    [0.5 + dx * r.cos() - dz * r.sin(), v[1], 0.5 + dx * r.sin() + dz * r.cos()]
                }),
            uv: flip_v(uv_rect(0.0, 0.0, 8.0, 16.0, 0)),
            texture_path: "blocks/brewing_stand.png".to_string(),
            tint_index: None,
        });
    }

    let bottle_texture = "blocks/brewing_stand_base.png";
    let bottle_slots = [
        ((meta & 0x1) != 0, [11.0, 2.0, 6.5], 0),
        ((meta & 0x2) != 0, [4.5, 2.0, 3.0], 1),
        ((meta & 0x4) != 0, [4.5, 2.0, 10.0], 3),
    ];
    for (present, origin, turns) in bottle_slots {
        if !present {
            continue;
        }
        let north = rotate_vertices_y(
            face_vertices(
                [origin[0], origin[1], origin[2]],
                [origin[0] + 3.0, origin[1] + 6.0, origin[2]],
                "north",
            )
            .unwrap(),
            turns,
        );
        let south = rotate_vertices_y(
            face_vertices(
                [origin[0], origin[1], origin[2]],
                [origin[0] + 3.0, origin[1] + 6.0, origin[2]],
                "south",
            )
            .unwrap(),
            turns,
        );
        out.push(IconQuad {
            vertices: north,
            uv: flip_v(uv_rect(0.0, 0.0, 3.0, 6.0, 0)),
            texture_path: bottle_texture.to_string(),
            tint_index: None,
        });
        out.push(IconQuad {
            vertices: south,
            uv: flip_v(uv_rect(3.0, 0.0, 0.0, 6.0, 0)),
            texture_path: bottle_texture.to_string(),
            tint_index: None,
        });
    }

    out
}

pub fn skull_display_quads(meta: u8) -> Vec<IconQuad> {
    let mut out = Vec::new();
    let facing = meta & 0x7;
    let (min, max) = match facing {
        2 => ([4.0, 4.0, 8.0], [12.0, 12.0, 16.0]),
        3 => ([4.0, 4.0, 0.0], [12.0, 12.0, 8.0]),
        4 => ([8.0, 4.0, 4.0], [16.0, 12.0, 12.0]),
        5 => ([0.0, 4.0, 4.0], [8.0, 12.0, 12.0]),
        _ => ([4.0, 0.0, 4.0], [12.0, 8.0, 12.0]),
    };
    let faces = [
        ("down", "blocks/head_player_bottom.png"),
        ("up", "blocks/head_player_top.png"),
        ("north", "blocks/head_player_front.png"),
        ("south", "blocks/head_player_back.png"),
        ("west", "blocks/head_player_left.png"),
        ("east", "blocks/head_player_right.png"),
    ];
    for (dir, texture) in faces {
        if let Some(vertices) = face_vertices(min, max, dir) {
            out.push(IconQuad {
                vertices,
                uv: [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                texture_path: texture.to_string(),
                tint_index: None,
            });
        }
    }
    out
}

pub fn sign_display_quads(block_id: u16, meta: u8) -> Vec<IconQuad> {
    let texture = "sign_entity.png";
    let mut out = Vec::new();
    if block_id == 63 {
        let mut board_quads = Vec::new();
        push_box_quads(
            &mut board_quads,
            texture,
            (64.0, 32.0),
            (0.0, 0.0),
            [0.0, 7.0, 7.0],
            [16.0, 8.0, 2.0],
            false,
        );
        let mut post_quads = Vec::new();
        push_box_quads(
            &mut post_quads,
            texture,
            (64.0, 32.0),
            (0.0, 14.0),
            [7.0, 0.0, 7.0],
            [2.0, 9.0, 2.0],
            false,
        );
        let angle = -(meta as f32) * std::f32::consts::TAU / 16.0;
        for quad in board_quads.into_iter().chain(post_quads) {
            out.push(IconQuad {
                vertices: rotate_vertices_around_y(
                    quad.vertices,
                    [0.5, 0.0, 0.5],
                    angle,
                ),
                uv: quad.uv,
                texture_path: quad.texture_path,
                tint_index: None,
            });
        }
    } else {
        let (origin, size) = match meta & 0x7 {
            2 => ([0.0, 4.5, 14.0], [16.0, 8.0, 2.0]),
            3 => ([0.0, 4.5, 0.0], [16.0, 8.0, 2.0]),
            4 => ([14.0, 4.5, 0.0], [2.0, 8.0, 16.0]),
            5 => ([0.0, 4.5, 0.0], [2.0, 8.0, 16.0]),
            _ => ([0.0, 4.5, 14.0], [16.0, 8.0, 2.0]),
        };
        push_box_quads(
            &mut out,
            texture,
            (64.0, 32.0),
            (0.0, 0.0),
            origin,
            size,
            false,
        );
    }
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
        117 => return Some(brewing_stand_display_quads(meta)),
        144 => return Some(skull_display_quads(meta)),
        63 | 68 => return Some(sign_display_quads(block_id, meta)),
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
