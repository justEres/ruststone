use std::collections::HashMap;

use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use rs_utils::ChunkData;

use crate::block_textures::{
    biome_tint, texture_for_face, texture_path, uv_for_texture, BiomeTint, Face, TextureKey,
};

const CHUNK_SIZE: i32 = 16;
const SECTION_HEIGHT: i32 = 16;
const WORLD_HEIGHT: i32 = 256;
const TEXTURE_BASE: &str = "texturepack/assets/minecraft/textures/blocks/";

#[derive(Resource, Default)]
pub struct ChunkUpdateQueue(pub Vec<ChunkData>);

#[derive(Resource, Default)]
pub struct ChunkRenderState {
    pub entries: HashMap<(i32, i32), ChunkEntry>,
}

pub struct ChunkEntry {
    pub entity: Entity,
    pub submeshes: HashMap<TextureKey, SubmeshEntry>,
}

pub struct SubmeshEntry {
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
    pub biomes: Option<Vec<u8>>,
}

impl ChunkColumn {
    fn new() -> Self {
        Self {
            full: false,
            sections: vec![None; 16],
            biomes: None,
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
    pub fn build_mesh_data(&self, use_greedy: bool) -> MeshBatch {
        if use_greedy {
            build_chunk_mesh_greedy(self, self.center_key.0, self.center_key.1)
        } else {
            build_chunk_mesh_culled(self, self.center_key.0, self.center_key.1)
        }
    }
}

#[derive(Default)]
pub struct MeshBatch {
    pub meshes: HashMap<TextureKey, MeshData>,
}

impl MeshBatch {
    pub fn ensure(&mut self, key: TextureKey) -> &mut MeshData {
        self.meshes.entry(key).or_insert_with(MeshData::empty)
    }
}

pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

impl MeshData {
    pub fn empty() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
        }
    }
}

#[derive(Resource)]
pub struct ChunkRenderAssets {
    pub materials: HashMap<TextureKey, Handle<StandardMaterial>>,
}

impl FromWorld for ChunkRenderAssets {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>().clone();
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let mut materials_map = HashMap::new();

        let mut register = |key: TextureKey| {
            let path = format!("{}{}", TEXTURE_BASE, texture_path(key));
            let texture: Handle<Image> =
                asset_server.load_with_settings(path, |settings: &mut ImageLoaderSettings| {
                    let mut sampler = ImageSamplerDescriptor::nearest();
                    sampler.address_mode_u = ImageAddressMode::Repeat;
                    sampler.address_mode_v = ImageAddressMode::Repeat;
                    sampler.address_mode_w = ImageAddressMode::Repeat;
                    settings.sampler = ImageSampler::Descriptor(sampler);
                });
            let mut material = StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: Some(texture),
                perceptual_roughness: 1.0,
                ..default()
            };
            if matches!(key, TextureKey::Water | TextureKey::Lava) {
                material.alpha_mode = AlphaMode::Blend;
                material.cull_mode = None;
                material.base_color = Color::srgba(1.0, 1.0, 1.0, 0.8);
            }
            let handle = materials.add(material);
            materials_map.insert(key, handle);
        };

        for key in [
            TextureKey::Stone,
            TextureKey::Dirt,
            TextureKey::GrassTop,
            TextureKey::GrassSide,
            TextureKey::Cobblestone,
            TextureKey::Planks,
            TextureKey::Bedrock,
            TextureKey::Sand,
            TextureKey::Gravel,
            TextureKey::GoldOre,
            TextureKey::IronOre,
            TextureKey::CoalOre,
            TextureKey::LogOak,
            TextureKey::LeavesOak,
            TextureKey::Sponge,
            TextureKey::Glass,
            TextureKey::LapisOre,
            TextureKey::LapisBlock,
            TextureKey::SandstoneTop,
            TextureKey::SandstoneSide,
            TextureKey::SandstoneBottom,
            TextureKey::NoteBlock,
            TextureKey::GoldBlock,
            TextureKey::IronBlock,
            TextureKey::Brick,
            TextureKey::TntTop,
            TextureKey::TntSide,
            TextureKey::TntBottom,
            TextureKey::MossyCobble,
            TextureKey::Obsidian,
            TextureKey::DiamondOre,
            TextureKey::DiamondBlock,
            TextureKey::CraftingTop,
            TextureKey::CraftingSide,
            TextureKey::CraftingFront,
            TextureKey::FurnaceTop,
            TextureKey::FurnaceSide,
            TextureKey::FurnaceFront,
            TextureKey::Ladder,
            TextureKey::CactusTop,
            TextureKey::CactusSide,
            TextureKey::CactusBottom,
            TextureKey::Clay,
            TextureKey::Snow,
            TextureKey::SnowBlock,
            TextureKey::Ice,
            TextureKey::SoulSand,
            TextureKey::Glowstone,
            TextureKey::Netherrack,
            TextureKey::Water,
            TextureKey::Lava,
        ] {
            register(key);
        }

        Self { materials: materials_map }
    }
}

pub fn apply_mesh_data(mesh: &mut Mesh, data: MeshData) {
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, data.positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, data.normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, data.uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, data.colors);
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

    if let Some(biomes) = chunk.biomes {
        column.biomes = Some(biomes);
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

fn build_chunk_mesh_culled(snapshot: &ChunkColumnSnapshot, chunk_x: i32, chunk_z: i32) -> MeshBatch {
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
                    if block_id == 0 {
                        continue;
                    }

                    let tint = biome_tint(biome_at(snapshot, chunk_x, chunk_z, x, z));

                    add_block_faces(
                        &mut batch,
                        snapshot,
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
    texture: TextureKey,
    tint_key: u8,
}

#[derive(Debug)]
struct GreedyQuad {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

fn build_chunk_mesh_greedy(snapshot: &ChunkColumnSnapshot, chunk_x: i32, chunk_z: i32) -> MeshBatch {
    let mut batch = MeshBatch::default();

    let Some(column) = snapshot.columns.get(&(chunk_x, chunk_z)) else {
        return batch;
    };

    for (section_y, section_opt) in column.sections.iter().enumerate() {
        let Some(section_blocks) = section_opt else {
            continue;
        };
        let base_y = section_y as i32 * SECTION_HEIGHT;

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
                        if block_id == 0 {
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
                        if block_at(snapshot, chunk_x, chunk_z, x + dx, base_y + y + dy, z + dz)
                            != 0
                        {
                            continue;
                        }

                        let texture = texture_for_face(block_id, face);
                        let biome_id = biome_at(snapshot, chunk_x, chunk_z, x, z);
                        let tint_key = if matches!(
                            texture,
                            TextureKey::GrassTop
                                | TextureKey::GrassSide
                                | TextureKey::LeavesOak
                                | TextureKey::Water
                        ) {
                            biome_id
                        } else {
                            0
                        };
                        let key = GreedyKey { texture, tint_key };

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
                        let tint = biome_tint(key.tint_key);
                        add_greedy_quad(
                            &mut batch,
                            face,
                            axis as i32,
                            base_y,
                            quad,
                            key.texture,
                            tint,
                        );
                    }
                }
            }
        }
    }

    batch
}

fn add_greedy_quad(
    batch: &mut MeshBatch,
    face: Face,
    axis: i32,
    base_y: i32,
    quad: GreedyQuad,
    texture: TextureKey,
    tint: BiomeTint,
) {
    let data = batch.ensure(texture);
    let base_index = data.positions.len() as u32;

    let u0 = quad.x as f32;
    let v0 = quad.y as f32;
    let u1 = u0 + quad.w as f32;
    let v1 = v0 + quad.h as f32;

    let (normal, verts) = match face {
        Face::PosY => {
            let y = (base_y + axis + 1) as f32;
            (
                [0.0, 1.0, 0.0],
                [
                    [u0, y, v0],
                    [u1, y, v0],
                    [u1, y, v1],
                    [u0, y, v1],
                ],
            )
        }
        Face::NegY => {
            let y = (base_y + axis) as f32;
            (
                [0.0, -1.0, 0.0],
                [
                    [u0, y, v1],
                    [u1, y, v1],
                    [u1, y, v0],
                    [u0, y, v0],
                ],
            )
        }
        Face::PosX => {
            let x = (axis + 1) as f32;
            let y0 = (base_y as f32) + v0;
            let y1 = (base_y as f32) + v1;
            (
                [1.0, 0.0, 0.0],
                [
                    [x, y0, u0],
                    [x, y0, u1],
                    [x, y1, u1],
                    [x, y1, u0],
                ],
            )
        }
        Face::NegX => {
            let x = axis as f32;
            let y0 = (base_y as f32) + v0;
            let y1 = (base_y as f32) + v1;
            (
                [-1.0, 0.0, 0.0],
                [
                    [x, y0, u1],
                    [x, y0, u0],
                    [x, y1, u0],
                    [x, y1, u1],
                ],
            )
        }
        Face::PosZ => {
            let z = (axis + 1) as f32;
            let y0 = (base_y as f32) + v0;
            let y1 = (base_y as f32) + v1;
            (
                [0.0, 0.0, 1.0],
                [
                    [u1, y0, z],
                    [u0, y0, z],
                    [u0, y1, z],
                    [u1, y1, z],
                ],
            )
        }
        Face::NegZ => {
            let z = axis as f32;
            let y0 = (base_y as f32) + v0;
            let y1 = (base_y as f32) + v1;
            (
                [0.0, 0.0, -1.0],
                [
                    [u0, y0, z],
                    [u1, y0, z],
                    [u1, y1, z],
                    [u0, y1, z],
                ],
            )
        }
    };

    for vert in verts {
        data.positions.push(vert);
        data.normals.push(normal);
    }

    let base_uvs = uv_for_texture(texture);
    for uv in base_uvs {
        data.uvs.push([uv[0] * quad.w as f32, uv[1] * quad.h as f32]);
    }
    let color = tint_color(texture, tint);
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

fn add_block_faces(
    batch: &mut MeshBatch,
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    block_id: u16,
    tint: BiomeTint,
) {
    let faces = [
        (Face::PosX, 1, 0, 0, [1.0, 0.0, 0.0], [
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0],
        ]),
        (Face::NegX, -1, 0, 0, [-1.0, 0.0, 0.0], [
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 1.0, 1.0],
        ]),
        (Face::PosY, 0, 1, 0, [0.0, 1.0, 0.0], [
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ]),
        (Face::NegY, 0, -1, 0, [0.0, -1.0, 0.0], [
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
        ]),
        (Face::PosZ, 0, 0, 1, [0.0, 0.0, 1.0], [
            [1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
        ]),
        (Face::NegZ, 0, 0, -1, [0.0, 0.0, -1.0], [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ]),
    ];

    for (face, dx, dy, dz, normal, verts) in faces {
        if block_at(snapshot, chunk_x, chunk_z, x + dx, y + dy, z + dz) != 0 {
            continue;
        }

        let texture = texture_for_face(block_id, face);
        let data = batch.ensure(texture);
        let base_index = data.positions.len() as u32;
        for vert in verts {
            data.positions
                .push([vert[0] + x as f32, vert[1] + y as f32, vert[2] + z as f32]);
            data.normals.push(normal);
        }
        let uvs = uv_for_texture(texture);
        data.uvs.extend_from_slice(&uvs);
        let color = tint_color(texture, tint);
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

fn tint_color(texture: TextureKey, tint: BiomeTint) -> [f32; 4] {
    match texture {
        TextureKey::GrassTop | TextureKey::GrassSide => tint.grass,
        TextureKey::LeavesOak => tint.foliage,
        TextureKey::Water => tint.water,
        _ => [1.0, 1.0, 1.0, 1.0],
    }
}

fn biome_at(snapshot: &ChunkColumnSnapshot, chunk_x: i32, chunk_z: i32, x: i32, z: i32) -> u8 {
    let Some(column) = snapshot.columns.get(&(chunk_x, chunk_z)) else {
        return 1;
    };
    let Some(biomes) = column.biomes.as_ref() else {
        return 1;
    };
    let idx = (z as usize & 15) * 16 + (x as usize & 15);
    *biomes.get(idx).unwrap_or(&1)
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
