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
    pub entries: HashMap<(i32, i32), ChunkEntry>,
}

pub struct ChunkEntry {
    pub entity: Entity,
    pub mesh: Handle<Mesh>,
}

#[derive(Resource, Default)]
pub struct ChunkStore {
    pub chunks: HashMap<(i32, i32), ChunkColumn>,
}

#[derive(Clone)]
pub struct ChunkColumn {
    pub full: bool,
    pub sections: Vec<Option<Vec<u16>>>,
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

#[derive(Clone)]
pub struct ChunkColumnSnapshot {
    pub center_key: (i32, i32),
    pub columns: HashMap<(i32, i32), ChunkColumn>,
}

impl ChunkColumnSnapshot {
    pub fn build_mesh_data(&self) -> MeshData {
        build_chunk_mesh(self, self.center_key.0, self.center_key.1)
    }
}

pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

impl MeshData {
    pub fn empty() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            indices: Vec::new(),
        }
    }
}

#[derive(Resource)]
pub struct ChunkRenderAssets {
    pub material: Handle<StandardMaterial>,
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

pub fn apply_mesh_data(mesh: &mut Mesh, data: MeshData) {
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, data.positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, data.normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, data.uvs);
    mesh.insert_indices(Indices::U32(data.indices));
}

pub fn build_mesh_from_data(data: MeshData) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    apply_mesh_data(&mut mesh, data);
    mesh
}

pub fn update_store(store: &mut ChunkStore, chunk: ChunkData) {
    let key = (chunk.x, chunk.z);
    let column = store.chunks.entry(key).or_insert_with(ChunkColumn::new);

    if chunk.full {
        column.set_full();
    }

    for section in chunk.sections {
        column.set_section(section.y, section.blocks);
    }
}

pub fn snapshot_for_chunk(store: &ChunkStore, key: (i32, i32)) -> ChunkColumnSnapshot {
    let mut columns = HashMap::new();
    for dz in -1..=1 {
        for dx in -1..=1 {
            let neighbor_key = (key.0 + dx, key.1 + dz);
            if let Some(column) = store.chunks.get(&neighbor_key) {
                columns.insert(neighbor_key, column.clone());
            }
        }
    }
    ChunkColumnSnapshot { center_key: key, columns }
}

fn build_chunk_mesh(snapshot: &ChunkColumnSnapshot, chunk_x: i32, chunk_z: i32) -> MeshData {
    let mut data = MeshData::empty();

    let Some(column) = snapshot.columns.get(&(chunk_x, chunk_z)) else {
        return data;
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
                    if block_id == 0 {
                        continue;
                    }

                    add_block_faces(
                        &mut data,
                        snapshot,
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

    data
}

fn add_block_faces(
    data: &mut MeshData,
    snapshot: &ChunkColumnSnapshot,
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
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ]),
        (0, -1, 0, [0.0, -1.0, 0.0], [
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
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
        if block_at(snapshot, chunk_x, chunk_z, x + dx, y + dy, z + dz) != 0 {
            continue;
        }

        let base_index = data.positions.len() as u32;
        for vert in verts {
            data.positions
                .push([vert[0] + x as f32, vert[1] + y as f32, vert[2] + z as f32]);
            data.normals.push(normal);
        }
        data.uvs.extend_from_slice(&[
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
        ]);
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

fn block_at(snapshot: &ChunkColumnSnapshot, chunk_x: i32, chunk_z: i32, x: i32, y: i32, z: i32) -> u16 {
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

    let Some(column) = snapshot.columns.get(&(target_chunk_x, target_chunk_z)) else {
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
