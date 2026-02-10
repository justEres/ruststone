use std::collections::HashMap;
use std::path::PathBuf;

use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, ShaderRef, TextureDimension, TextureFormat,
};
use image::{DynamicImage, ImageBuffer, Rgba, imageops};
use rs_utils::ChunkData;

use crate::block_textures::{
    ATLAS_COLUMNS, ATLAS_ROWS, ATLAS_TEXTURES, BiomeTint, Face, TextureKey, atlas_tile_origin,
    biome_tint, is_transparent_texture, texture_for_face, texture_path, uv_for_texture,
};

const CHUNK_SIZE: i32 = 16;
const SECTION_HEIGHT: i32 = 16;
const WORLD_HEIGHT: i32 = 256;
const TEXTURE_BASE: &str = "texturepack/assets/minecraft/textures/blocks/";
const ATLAS_PBR_SHADER_PATH: &str = "shaders/atlas_pbr.wgsl";

pub type ChunkAtlasMaterial = ExtendedMaterial<StandardMaterial, AtlasTextureExtension>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct AtlasTextureExtension {
    #[texture(100)]
    #[sampler(101)]
    pub atlas: Handle<Image>,
}

impl MaterialExtension for AtlasTextureExtension {
    fn fragment_shader() -> ShaderRef {
        ATLAS_PBR_SHADER_PATH.into()
    }

    fn deferred_fragment_shader() -> ShaderRef {
        ATLAS_PBR_SHADER_PATH.into()
    }
}

#[derive(Resource, Default)]
pub struct ChunkUpdateQueue(pub Vec<ChunkData>);

#[derive(Resource, Default)]
pub struct ChunkRenderState {
    pub entries: HashMap<(i32, i32), ChunkEntry>,
}

pub struct ChunkEntry {
    pub entity: Entity,
    pub submeshes: HashMap<MaterialGroup, SubmeshEntry>,
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

pub struct MeshBatch {
    pub opaque: MeshData,
    pub transparent: MeshData,
}

impl MeshBatch {
    pub fn data_for(&mut self, key: TextureKey) -> &mut MeshData {
        if is_transparent_texture(key) {
            &mut self.transparent
        } else {
            &mut self.opaque
        }
    }
}

impl Default for MeshBatch {
    fn default() -> Self {
        Self {
            opaque: MeshData::empty(),
            transparent: MeshData::empty(),
        }
    }
}

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
pub enum MaterialGroup {
    Opaque,
    Transparent,
}

pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub uvs_b: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    pub bounds_min: Option<Vec3>,
    pub bounds_max: Option<Vec3>,
}

impl MeshData {
    pub fn empty() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            uvs_b: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
            bounds_min: None,
            bounds_max: None,
        }
    }

    pub fn push_pos(&mut self, pos: [f32; 3]) {
        let v = Vec3::new(pos[0], pos[1], pos[2]);
        match (self.bounds_min.as_mut(), self.bounds_max.as_mut()) {
            (Some(min), Some(max)) => {
                *min = min.min(v);
                *max = max.max(v);
            }
            _ => {
                self.bounds_min = Some(v);
                self.bounds_max = Some(v);
            }
        }
        self.positions.push(pos);
    }

    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        match (self.bounds_min, self.bounds_max) {
            (Some(min), Some(max)) => Some((min, max)),
            _ => None,
        }
    }
}

#[derive(Resource)]
pub struct ChunkRenderAssets {
    pub opaque_material: Handle<ChunkAtlasMaterial>,
    pub transparent_material: Handle<ChunkAtlasMaterial>,
    pub atlas: Handle<Image>,
}

impl FromWorld for ChunkRenderAssets {
    fn from_world(world: &mut World) -> Self {
        let mut atlas_image = load_or_build_atlas();
        let mut sampler = ImageSamplerDescriptor::nearest();
        sampler.address_mode_u = ImageAddressMode::ClampToEdge;
        sampler.address_mode_v = ImageAddressMode::ClampToEdge;
        sampler.address_mode_w = ImageAddressMode::ClampToEdge;
        atlas_image.sampler = ImageSampler::Descriptor(sampler);
        let atlas_handle = {
            let mut images = world.resource_mut::<Assets<Image>>();
            images.add(atlas_image)
        };
        let mut materials = world.resource_mut::<Assets<ChunkAtlasMaterial>>();

        let opaque_material = materials.add(ChunkAtlasMaterial {
            base: StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: None,
                perceptual_roughness: 1.0,
                ..default()
            },
            extension: AtlasTextureExtension {
                atlas: atlas_handle.clone(),
            },
        });
        let transparent_material = materials.add(ChunkAtlasMaterial {
            base: StandardMaterial {
                base_color: Color::srgba(1.0, 1.0, 1.0, 0.8),
                base_color_texture: None,
                perceptual_roughness: 1.0,
                alpha_mode: AlphaMode::Blend,
                cull_mode: None,
                ..default()
            },
            extension: AtlasTextureExtension {
                atlas: atlas_handle.clone(),
            },
        });

        Self {
            opaque_material,
            transparent_material,
            atlas: atlas_handle,
        }
    }
}

fn assets_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rs-client/assets")
}

fn atlas_cache_path() -> PathBuf {
    assets_root().join("texturepack/atlas_cache_v2.png")
}

fn texture_root_path() -> PathBuf {
    assets_root().join(TEXTURE_BASE)
}

fn load_or_build_atlas() -> Image {
    let cache_path = atlas_cache_path();
    if let Ok(img) = image::open(&cache_path) {
        return bevy_image_from_rgba(img);
    }

    let textures_root = texture_root_path();
    let mut tile_size = None;
    let mut atlas = None::<ImageBuffer<Rgba<u8>, Vec<u8>>>;

    for (idx, key) in ATLAS_TEXTURES.iter().enumerate() {
        let texture_path = textures_root.join(texture_path(*key));
        let img = image::open(&texture_path).unwrap_or_else(|_| missing_texture_image());
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let size = tile_size.get_or_insert((w, h));
        let (tile_w, tile_h) = *size;
        let rgba = if w != tile_w || h != tile_h {
            imageops::resize(&rgba, tile_w, tile_h, imageops::Nearest)
        } else {
            rgba
        };

        let atlas_buf = atlas.get_or_insert_with(|| {
            ImageBuffer::from_pixel(
                tile_w * ATLAS_COLUMNS,
                tile_h * ATLAS_ROWS,
                Rgba([0, 0, 0, 0]),
            )
        });
        let col = (idx as u32) % ATLAS_COLUMNS;
        let row = (idx as u32) / ATLAS_COLUMNS;
        let x = col * tile_w;
        let y = row * tile_h;
        imageops::overlay(atlas_buf, &rgba, x.into(), y.into());
    }

    let atlas = atlas
        .unwrap_or_else(|| ImageBuffer::from_pixel(ATLAS_COLUMNS, ATLAS_ROWS, Rgba([0, 0, 0, 0])));

    if let Some(parent) = cache_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = atlas.save(&cache_path);

    bevy_image_from_rgba(DynamicImage::ImageRgba8(atlas))
}

fn bevy_image_from_rgba(img: DynamicImage) -> Image {
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let data = rgba.into_raw();
    let mut image = Image::new_fill(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.data = Some(data);
    image
}

fn missing_texture_image() -> DynamicImage {
    let mut img = ImageBuffer::from_pixel(16, 16, Rgba([255, 0, 255, 255]));
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        if (x + y) % 2 == 0 {
            *pixel = Rgba([0, 0, 0, 255]);
        }
    }
    DynamicImage::ImageRgba8(img)
}

pub fn apply_mesh_data(mesh: &mut Mesh, data: MeshData) {
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, data.positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, data.normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, data.uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, data.uvs_b);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, data.colors);
    mesh.insert_indices(Indices::U32(data.indices));
}

pub fn build_mesh_from_data(data: MeshData) -> (Mesh, Option<(Vec3, Vec3)>) {
    let bounds = data.bounds();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    apply_mesh_data(&mut mesh, data);
    (mesh, bounds)
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
    ChunkColumnSnapshot {
        center_key: key,
        columns,
    }
}

fn build_chunk_mesh_culled(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
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

fn build_chunk_mesh_greedy(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
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
                        let neighbor =
                            block_at(snapshot, chunk_x, chunk_z, x + dx, base_y + y + dy, z + dz);
                        if face_is_occluded(block_id, neighbor) {
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
    let data = batch.data_for(texture);
    let base_index = data.positions.len() as u32;
    let tile_origin = atlas_tile_origin(texture);

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

    let base_uvs = uv_for_texture(texture);
    for uv in base_uvs {
        data.uvs
            .push([uv[0] * quad.w as f32, uv[1] * quad.h as f32]);
        data.uvs_b.push(tile_origin);
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
        if face_is_occluded(block_id, neighbor) {
            continue;
        }

        let texture = texture_for_face(block_id, face);
        let data = batch.data_for(texture);
        let base_index = data.positions.len() as u32;
        for vert in verts {
            data.push_pos([vert[0] + x as f32, vert[1] + y as f32, vert[2] + z as f32]);
            data.normals.push(normal);
        }
        let uvs = uv_for_texture(texture);
        data.uvs.extend_from_slice(&uvs);
        let tile_origin = atlas_tile_origin(texture);
        data.uvs_b.extend_from_slice(&[tile_origin; 4]);
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

fn block_at(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
) -> u16 {
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

fn is_liquid(block_id: u16) -> bool {
    matches!(block_id, 8 | 9 | 10 | 11)
}

fn face_is_occluded(block_id: u16, neighbor_id: u16) -> bool {
    if neighbor_id == 0 {
        return false;
    }
    if is_liquid(neighbor_id) {
        return is_liquid(block_id);
    }
    true
}
