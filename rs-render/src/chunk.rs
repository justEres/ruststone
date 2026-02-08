use std::collections::HashMap;

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use rs_utils::ChunkData;

const CHUNK_SIZE: i32 = 16;
const SECTION_HEIGHT: i32 = 16;
const WORLD_HEIGHT: i32 = 256;

#[derive(Resource, Default)]
pub struct ChunkUpdateQueue(pub Vec<ChunkData>);

#[derive(Resource, Default)]
pub struct ChunkRenderState {
    entries: HashMap<(i32, i32), ChunkEntry>,
}

struct ChunkEntry {
    entity: Entity,
    mesh: Handle<Mesh>,
}

#[derive(Resource)]
pub struct ChunkRenderAssets {
    material: Handle<StandardMaterial>,
}

impl FromWorld for ChunkRenderAssets {
    fn from_world(world: &mut World) -> Self {
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.55, 0.7, 0.55),
            perceptual_roughness: 1.0,
            ..default()
        });
        Self { material }
    }
}

pub fn apply_chunk_updates(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    assets: Res<ChunkRenderAssets>,
    mut queue: ResMut<ChunkUpdateQueue>,
    mut state: ResMut<ChunkRenderState>,
) {
    if queue.0.is_empty() {
        return;
    }

    for chunk in queue.0.drain(..) {
        let key = (chunk.x, chunk.z);
        let mesh = build_chunk_mesh(&chunk);
        if let Some(entry) = state.entries.get_mut(&key) {
            if let Some(existing) = meshes.get_mut(&entry.mesh) {
                *existing = mesh;
            } else {
                let handle = meshes.add(mesh);
                commands.entity(entry.entity).insert(Mesh3d(handle.clone()));
                entry.mesh = handle;
            }
        } else {
            let handle = meshes.add(mesh);
            let entity = commands
                .spawn((
                    Mesh3d(handle.clone()),
                    MeshMaterial3d(assets.material.clone()),
                    Transform::from_xyz(
                        (chunk.x * CHUNK_SIZE) as f32,
                        0.0,
                        (chunk.z * CHUNK_SIZE) as f32,
                    ),
                    GlobalTransform::default(),
                ))
                .id();

            state.entries.insert(key, ChunkEntry { entity, mesh: handle });
        }
    }
}

fn build_chunk_mesh(chunk: &ChunkData) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let mut section_map: Vec<Option<&[u8]>> = vec![None; 16];
    for section in &chunk.sections {
        section_map[section.y as usize] = Some(&section.blocks);
    }

    for section in &chunk.sections {
        let base_y = section.y as i32 * SECTION_HEIGHT;
        for y in 0..SECTION_HEIGHT {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let idx = (y * CHUNK_SIZE * CHUNK_SIZE + z * CHUNK_SIZE + x) as usize;
                    let block_id = section.blocks[idx];
                    if block_id == 0 {
                        continue;
                    }

                    let wx = x;
                    let wy = base_y + y;
                    let wz = z;

                    add_block_faces(
                        &mut positions,
                        &mut normals,
                        &mut uvs,
                        &mut indices,
                        &section_map,
                        wx,
                        wy,
                        wz,
                    );
                }
            }
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn add_block_faces(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    section_map: &[Option<&[u8]>],
    x: i32,
    y: i32,
    z: i32,
) {
    let faces = [
        (1, 0, 0, [1.0, 0.0, 0.0], [
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
        ]),
        (-1, 0, 0, [-1.0, 0.0, 0.0], [
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 1.0],
        ]),
        (0, 1, 0, [0.0, 1.0, 0.0], [
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ]),
        (0, -1, 0, [0.0, -1.0, 0.0], [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
        ]),
        (0, 0, 1, [0.0, 0.0, 1.0], [
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
        ]),
        (0, 0, -1, [0.0, 0.0, -1.0], [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ]),
    ];

    for (dx, dy, dz, normal, verts) in faces {
        if block_at(section_map, x + dx, y + dy, z + dz) != 0 {
            continue;
        }

        let base_index = positions.len() as u32;
        for vert in verts {
            positions.push([vert[0] + x as f32, vert[1] + y as f32, vert[2] + z as f32]);
            normals.push(normal);
        }
        uvs.extend_from_slice(&[
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
        ]);
        indices.extend_from_slice(&[
            base_index,
            base_index + 1,
            base_index + 2,
            base_index,
            base_index + 2,
            base_index + 3,
        ]);
    }
}

fn block_at(section_map: &[Option<&[u8]>], x: i32, y: i32, z: i32) -> u8 {
    if x < 0 || x >= CHUNK_SIZE || z < 0 || z >= CHUNK_SIZE || y < 0 || y >= WORLD_HEIGHT {
        return 0;
    }

    let section_index = (y / SECTION_HEIGHT) as usize;
    let local_y = (y % SECTION_HEIGHT) as usize;
    let local_x = x as usize;
    let local_z = z as usize;

    let Some(blocks) = section_map.get(section_index).and_then(|v| *v) else {
        return 0;
    };
    let idx = local_y * 16 * 16 + local_z * 16 + local_x;
    blocks[idx]
}
