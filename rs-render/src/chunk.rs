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

#[derive(Resource, Default)]
pub struct ChunkStore {
    chunks: HashMap<(i32, i32), ChunkColumn>,
}

#[derive(Clone)]
struct ChunkColumn {
    full: bool,
    sections: Vec<Option<Vec<u16>>>,
}

impl ChunkColumn {
    fn new() -> Self {
        Self {
            full: false,
            sections: vec![None; 16],
        }
    }

    fn set_full(&mut self) {
        self.full = true;
        for section in &mut self.sections {
            if section.is_none() {
                *section = Some(vec![0u16; 4096]);
            }
        }
    }

    fn set_section(&mut self, y: u8, blocks: Vec<u16>) {
        let idx = y as usize;
        if idx >= self.sections.len() {
            return;
        }
        self.sections[idx] = Some(blocks);
    }
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
    mut store: ResMut<ChunkStore>,
) {
    if queue.0.is_empty() {
        return;
    }

    let mut updated_keys = Vec::new();

    for chunk in queue.0.drain(..) {
        let key = (chunk.x, chunk.z);
        let column = store
            .chunks
            .entry(key)
            .or_insert_with(ChunkColumn::new);

        if chunk.full {
            column.set_full();
        }

        for section in chunk.sections {
            column.set_section(section.y, section.blocks);
        }

        updated_keys.push(key);
    }

    for key in updated_keys {
        let Some(column) = store.chunks.get(&key) else {
            continue;
        };
        let mesh = build_chunk_mesh(&store, key.0, key.1, column);

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
                        (key.0 * CHUNK_SIZE) as f32,
                        0.0,
                        (key.1 * CHUNK_SIZE) as f32,
                    ),
                    GlobalTransform::default(),
                ))
                .id();

            state.entries.insert(key, ChunkEntry { entity, mesh: handle });
        }
    }
}

fn build_chunk_mesh(store: &ChunkStore, chunk_x: i32, chunk_z: i32, column: &ChunkColumn) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

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
                    if block_id == 0 {
                        continue;
                    }

                    add_block_faces(
                        &mut positions,
                        &mut normals,
                        &mut uvs,
                        &mut indices,
                        store,
                        chunk_x,
                        chunk_z,
                        x,
                        base_y + y,
                        z,
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
    store: &ChunkStore,
    chunk_x: i32,
    chunk_z: i32,
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
        if block_at(store, chunk_x, chunk_z, x + dx, y + dy, z + dz) != 0 {
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
            base_index + 2,
            base_index + 1,
            base_index,
            base_index + 3,
            base_index + 2,
        ]);
    }
}

fn block_at(store: &ChunkStore, chunk_x: i32, chunk_z: i32, x: i32, y: i32, z: i32) -> u16 {
    if y < 0 || y >= WORLD_HEIGHT {
        return 0;
    }

    let mut target_chunk_x = chunk_x;
    let mut target_chunk_z = chunk_z;
    let mut local_x = x;
    let mut local_z = z;

    if local_x < 0 {
        target_chunk_x -= 1;
        local_x += CHUNK_SIZE;
    } else if local_x >= CHUNK_SIZE {
        target_chunk_x += 1;
        local_x -= CHUNK_SIZE;
    }

    if local_z < 0 {
        target_chunk_z -= 1;
        local_z += CHUNK_SIZE;
    } else if local_z >= CHUNK_SIZE {
        target_chunk_z += 1;
        local_z -= CHUNK_SIZE;
    }

    let Some(column) = store.chunks.get(&(target_chunk_x, target_chunk_z)) else {
        return 1;
    };

    let section_index = (y / SECTION_HEIGHT) as usize;
    let local_y = (y % SECTION_HEIGHT) as usize;

    let Some(section) = column.sections.get(section_index).and_then(|v| v.as_ref()) else {
        return if column.full { 0 } else { 1 };
    };

    let idx = local_y * 16 * 16 + local_z as usize * 16 + local_x as usize;
    section[idx]
}
