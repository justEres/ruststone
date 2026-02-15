use bevy::prelude::*;
use bevy::render::mesh::Indices;
use bevy::render::mesh::PrimitiveTopology;
use bevy::render::render_asset::RenderAssetUsages;

use super::{CubeDef, EntityTexturePath, ModelDef, PartDef};

const PX: f32 = 1.0 / 16.0;

#[derive(Debug, Clone)]
pub struct SpawnedModel {
    pub root: Entity,
    /// Bevy entities for each part, in the same order as `model.parts`.
    pub parts: Vec<Entity>,
}

pub fn part_mesh(model: &ModelDef, part: &PartDef) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for cube in part.cubes {
        add_cube(
            model,
            cube,
            &mut positions,
            &mut normals,
            &mut uvs,
            &mut indices,
        );
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

pub fn spawn_model(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    model: &'static ModelDef,
    texture_path: &'static str,
) -> SpawnedModel {
    let root = commands
        .spawn((
            Name::new("EntityModelRoot"),
            Transform::from_translation(Vec3::new(
                model.root_offset_px[0] * PX,
                model.root_offset_px[1] * PX,
                model.root_offset_px[2] * PX,
            )),
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ))
        .id();

    let mut part_entities: Vec<Entity> = vec![Entity::PLACEHOLDER; model.parts.len()];

    // First spawn all part pivots.
    for (idx, part) in model.parts.iter().enumerate() {
        let pivot = Vec3::new(part.pivot[0] * PX, -part.pivot[1] * PX, part.pivot[2] * PX);
        let e = commands
            .spawn((
                Name::new(format!("EntityModelPart[{}]", part.name)),
                Transform::from_translation(pivot),
                GlobalTransform::default(),
                Visibility::Visible,
                InheritedVisibility::default(),
                ViewVisibility::default(),
            ))
            .id();
        part_entities[idx] = e;
    }

    // Then attach to the appropriate parent and spawn meshes.
    for (idx, part) in model.parts.iter().enumerate() {
        let part_entity = part_entities[idx];
        let parent_entity = part
            .parent
            .and_then(|p| part_entities.get(p).copied())
            .unwrap_or(root);
        commands.entity(parent_entity).add_child(part_entity);

        if part.cubes.is_empty() {
            continue;
        }
        let mesh = meshes.add(part_mesh(model, part));
        let mesh_entity = commands
            .spawn((
                Name::new(format!("EntityModelMesh[{}]", part.name)),
                Mesh3d(mesh),
                MeshMaterial3d(material.clone()),
                Transform::IDENTITY,
                GlobalTransform::default(),
                Visibility::Visible,
                InheritedVisibility::default(),
                ViewVisibility::default(),
                EntityTexturePath(texture_path),
            ))
            .id();
        commands.entity(part_entity).add_child(mesh_entity);
    }

    SpawnedModel {
        root,
        parts: part_entities,
    }
}

fn add_cube(
    model: &ModelDef,
    cube: &CubeDef,
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
) {
    let [tex_w, tex_h] = model.tex_size;
    let [u, v] = cube.uv;
    let [x, y, z] = cube.from;
    let [w, h, d] = cube.size;
    let inf = cube.inflate;

    // Vanilla ModelBox expands by `inflate` on all sides. Y is "down" in model coordinates.
    let mut x1 = x - inf;
    let y1 = y - inf;
    let z1 = z - inf;
    let mut x2 = x + w + inf;
    let y2 = y + h + inf;
    let z2 = z + d + inf;

    // Mirror flips X and then flips faces.
    let mirrored = cube.mirror;
    if mirrored {
        std::mem::swap(&mut x1, &mut x2);
    }

    // Convert to bevy units (1px=1/16) and flip Y (Minecraft model space uses +Y down).
    let p = |xx: f32, yy: f32, zz: f32| -> [f32; 3] { [xx * PX, -yy * PX, zz * PX] };

    // Match ModelBox's 8 base vertices naming.
    let v7 = p(x1, y1, z1);
    let v0 = p(x2, y1, z1);
    let v1 = p(x2, y2, z1);
    let v2 = p(x1, y2, z1);
    let v3 = p(x1, y1, z2);
    let v4 = p(x2, y1, z2);
    let v5 = p(x2, y2, z2);
    let v6 = p(x1, y2, z2);

    // `TexturedQuad` assigns UVs to 4 vertices as:
    // 0: (u2, v1), 1: (u1, v1), 2: (u1, v2), 3: (u2, v2)
    let uv4 = |u1: u32, v1: u32, u2: u32, v2: u32| -> [[f32; 2]; 4] {
        let to_uv = |uu: u32, vv: u32| -> [f32; 2] {
            let u = uu as f32 / tex_w as f32;
            // Bevy/WGPU uses top-left UV convention with our texture data.
            let v = vv as f32 / tex_h as f32;
            [u, v]
        };
        [to_uv(u2, v1), to_uv(u1, v1), to_uv(u1, v2), to_uv(u2, v2)]
    };

    let (w_i, h_i, d_i) = (w as u32, h as u32, d as u32);

    // Face UV bounds from vanilla `ModelBox` ctor.
    let east = uv4(u + d_i + w_i, v + d_i, u + d_i + w_i + d_i, v + d_i + h_i);
    let west = uv4(u, v + d_i, u + d_i, v + d_i + h_i);
    let top = uv4(u + d_i, v, u + d_i + w_i, v + d_i);
    let bottom = uv4(u + d_i + w_i, v + d_i, u + d_i + w_i + w_i, v);
    let north = uv4(u + d_i, v + d_i, u + d_i + w_i, v + d_i + h_i);
    let south = uv4(
        u + d_i + w_i + d_i,
        v + d_i,
        u + d_i + w_i + d_i + w_i,
        v + d_i + h_i,
    );

    // Quads in the exact vertex order from vanilla `ModelBox`.
    add_quad(
        positions,
        normals,
        uvs,
        indices,
        [v4, v0, v1, v5],
        [1.0, 0.0, 0.0],
        east,
        mirrored,
    ); // +X
    add_quad(
        positions,
        normals,
        uvs,
        indices,
        [v7, v3, v6, v2],
        [-1.0, 0.0, 0.0],
        west,
        mirrored,
    ); // -X
    add_quad(
        positions,
        normals,
        uvs,
        indices,
        [v4, v3, v7, v0],
        [0.0, 1.0, 0.0],
        top,
        mirrored,
    ); // +Y in model space (down) => this is "bottom" in bevy, but normals are fixed by winding.
    add_quad(
        positions,
        normals,
        uvs,
        indices,
        [v1, v2, v6, v5],
        [0.0, -1.0, 0.0],
        bottom,
        mirrored,
    );
    add_quad(
        positions,
        normals,
        uvs,
        indices,
        [v0, v7, v2, v1],
        [0.0, 0.0, -1.0],
        north,
        mirrored,
    );
    add_quad(
        positions,
        normals,
        uvs,
        indices,
        [v3, v4, v5, v6],
        [0.0, 0.0, 1.0],
        south,
        mirrored,
    );
}

fn add_quad(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    mut verts: [[f32; 3]; 4],
    normal: [f32; 3],
    mut uv: [[f32; 2]; 4],
    mirrored: bool,
) {
    if mirrored {
        verts.reverse();
        uv.reverse();
    }

    // Ensure both triangles are consistently front-facing.
    let a = Vec3::from_array(verts[0]);
    let b = Vec3::from_array(verts[1]);
    let c = Vec3::from_array(verts[2]);
    let actual = (b - a).cross(c - a);
    let expected = Vec3::from_array(normal);
    if actual.dot(expected) < 0.0 {
        verts = [verts[0], verts[3], verts[2], verts[1]];
        uv = [uv[0], uv[3], uv[2], uv[1]];
    }

    let base = positions.len() as u32;
    for i in 0..4 {
        positions.push(verts[i]);
        normals.push(normal);
        uvs.push(uv[i]);
    }
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}
