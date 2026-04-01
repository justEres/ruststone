use std::collections::HashMap;
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::pbr::{ExtendedMaterial, MaterialExtension, OpaqueRendererMethod};
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, ShaderRef, ShaderType, TextureDimension, TextureFormat,
};
use image::{DynamicImage, ImageBuffer, Rgba, imageops};
use rs_utils::{
    BlockModelKind, BlockUpdate, ChunkData, block_model_kind, block_state_id, block_state_meta,
    ruststone_assets_root, texturepack_minecraft_root,
};

use crate::block_models::{BlockModelResolver, default_model_roots};
use crate::block_textures::{
    ATLAS_COLUMNS, ATLAS_ROWS, ATLAS_TILE_CAPACITY, AtlasBlockMapping, BiomeTint,
    BiomeTintResolver, Face, TintClass, atlas_tile_origin, build_block_texture_mapping,
    classify_tint, is_leaves_block, is_transparent_block, uv_for_texture,
};
use crate::debug::{RenderDebugSettings, ShadingModel, VanillaBlockShadowMode};
use crate::lighting::{lighting_uniform_for_mode, uses_shadowed_pbr_path};

mod assets;
mod ao;
mod custom;
mod custom_shapes;
mod geometry;
mod mesh;
mod sampling;
mod shading;
mod store;
mod tint;
mod visibility;
#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use assets::{apply_mesh_data, build_mesh_from_data};
pub use store::{apply_block_update, snapshot_for_chunk, update_store};

use ao::*;
use custom::*;
use custom_shapes::*;
use geometry::*;
use mesh::*;
use sampling::*;
use shading::*;
use store::build_chunk_occlusion_data;
use tint::*;
use visibility::*;

const CHUNK_SIZE: i32 = 16;
const SECTION_HEIGHT: i32 = 16;
const WORLD_HEIGHT: i32 = 256;
const TEXTURE_BASE: &str = "texturepack/assets/minecraft/textures/blocks/";
const ATLAS_PBR_SHADER_PATH: &str = "shaders/atlas_pbr.wgsl";
const ATLAS_UV_PACK_SCALE: f32 = 1024.0;

pub type ChunkAtlasMaterial = ExtendedMaterial<StandardMaterial, AtlasTextureExtension>;

#[derive(Clone, Copy, Debug, Reflect, ShaderType)]
pub struct AtlasLightingUniform {
    pub sun_dir_and_strength: Vec4,
    pub ambient_and_fog: Vec4,
    pub quality_and_water: Vec4,
    pub color_grading: Vec4,
    pub vanilla_light: Vec4,
    pub vanilla_shadow: Vec4,
    pub water_effects: Vec4,
    pub water_controls: Vec4,
    pub water_extra: Vec4,
    pub ssr_params: Vec4,
    pub debug_flags: Vec4,
    pub grass_overlay_info: Vec4,
    pub reflection_view_proj: Mat4,
}

#[derive(Clone, Copy, Debug)]
pub struct VanillaBakeSettings {
    pub shading_model: ShadingModel,
    pub sky_light_strength: f32,
    pub block_light_strength: f32,
    pub face_shading_strength: f32,
    pub ambient_floor: f32,
    pub light_curve: f32,
    pub foliage_tint_strength: f32,
    pub block_shadow_mode: VanillaBlockShadowMode,
    pub block_shadow_strength: f32,
    pub sun_trace_samples: u8,
    pub sun_trace_distance: f32,
    pub top_face_sun_bias: f32,
    pub ao_shadow_blend: f32,
}

impl VanillaBakeSettings {
    pub fn from_render_settings(settings: &RenderDebugSettings) -> Self {
        Self {
            shading_model: settings.shading_model,
            sky_light_strength: settings.vanilla_sky_light_strength,
            block_light_strength: settings.vanilla_block_light_strength,
            face_shading_strength: settings.vanilla_face_shading_strength,
            ambient_floor: settings.vanilla_ambient_floor,
            light_curve: settings.vanilla_light_curve,
            foliage_tint_strength: settings.vanilla_foliage_tint_strength,
            block_shadow_mode: settings.vanilla_block_shadow_mode,
            block_shadow_strength: settings.vanilla_block_shadow_strength,
            sun_trace_samples: settings.vanilla_sun_trace_samples,
            sun_trace_distance: settings.vanilla_sun_trace_distance,
            top_face_sun_bias: settings.vanilla_top_face_sun_bias,
            ao_shadow_blend: settings.vanilla_ao_shadow_blend,
        }
    }
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct AtlasTextureExtension {
    #[texture(100)]
    #[sampler(101)]
    pub atlas: Handle<Image>,
    #[texture(103, dimension = "cube")]
    #[sampler(104)]
    pub skybox: Handle<Image>,
    #[uniform(102)]
    pub lighting: AtlasLightingUniform,
}

impl MaterialExtension for AtlasTextureExtension {
    fn fragment_shader() -> ShaderRef {
        ATLAS_PBR_SHADER_PATH.into()
    }

    fn prepass_fragment_shader() -> ShaderRef {
        ATLAS_PBR_SHADER_PATH.into()
    }

    fn deferred_fragment_shader() -> ShaderRef {
        ATLAS_PBR_SHADER_PATH.into()
    }
}

#[derive(Clone)]
pub enum WorldUpdate {
    Reset,
    UnloadChunk(i32, i32),
    ChunkData(ChunkData),
    BlockUpdate(BlockUpdate),
}

#[derive(Resource, Default)]
pub struct ChunkUpdateQueue(pub Vec<WorldUpdate>);

#[derive(Resource, Default)]
pub struct PendingChunkRemesh {
    pub keys: std::collections::HashSet<(i32, i32)>,
}

#[derive(Resource, Default)]
pub struct ChunkRenderState {
    pub entries: HashMap<(i32, i32), ChunkEntry>,
    pub occlusion_revision: u64,
}

pub struct ChunkEntry {
    pub entity: Entity,
    pub submeshes: HashMap<SubmeshKey, SubmeshEntry>,
    pub occlusion: ChunkOcclusionData,
}

pub struct SubmeshEntry {
    pub entity: Entity,
    pub mesh: Handle<Mesh>,
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct SubmeshKey {
    pub group: MaterialGroup,
    pub section: u8,
}

#[derive(Resource, Default)]
pub struct ChunkStore {
    pub chunks: HashMap<(i32, i32), ChunkColumn>,
}

#[derive(Clone)]
pub struct ChunkColumn {
    pub full: bool,
    pub sections: Vec<Option<Vec<u16>>>,
    pub block_light_sections: Vec<Option<Vec<u8>>>,
    pub sky_light_sections: Vec<Option<Vec<u8>>>,
    pub biomes: Option<Vec<u8>>,
}

impl ChunkColumn {
    fn new() -> Self {
        Self {
            full: false,
            sections: vec![None; 16],
            block_light_sections: vec![None; 16],
            sky_light_sections: vec![None; 16],
            biomes: None,
        }
    }

    fn set_full(&mut self) {
        self.full = true;
        for idx in 0..self.sections.len() {
            self.sections[idx] = Some(vec![0u16; 4096]);
            self.block_light_sections[idx] = Some(vec![0u8; 4096]);
            self.sky_light_sections[idx] = Some(vec![15u8; 4096]);
        }
    }

    fn set_section(
        &mut self,
        y: u8,
        blocks: Vec<u16>,
        block_light: Vec<u8>,
        sky_light: Option<Vec<u8>>,
    ) {
        let idx = y as usize;
        if idx >= self.sections.len() {
            return;
        }
        self.sections[idx] = Some(blocks);
        self.block_light_sections[idx] = Some(block_light);
        self.sky_light_sections[idx] = sky_light;
    }

    fn set_block(&mut self, local_x: usize, y: i32, local_z: usize, block_id: u16) {
        if !(0..WORLD_HEIGHT).contains(&y) {
            return;
        }
        let section_index = (y / SECTION_HEIGHT) as usize;
        let local_y = (y % SECTION_HEIGHT) as usize;
        if section_index >= self.sections.len() {
            return;
        }
        let section = self.sections[section_index].get_or_insert_with(|| vec![0; 16 * 16 * 16]);
        let idx = local_y * 16 * 16 + local_z * 16 + local_x;
        if let Some(slot) = section.get_mut(idx) {
            *slot = block_id;
        }
    }
}

#[derive(Clone)]
pub struct ChunkColumnSnapshot {
    pub center_key: (i32, i32),
    pub columns: HashMap<(i32, i32), ChunkColumn>,
}

impl ChunkColumnSnapshot {
    pub fn build_mesh_data(
        &self,
        use_greedy: bool,
        leaf_depth_layer_faces: bool,
        voxel_ao_enabled: bool,
        voxel_ao_strength: f32,
        voxel_ao_cutout: bool,
        barrier_billboard: bool,
        vanilla_bake: VanillaBakeSettings,
        texture_mapping: &AtlasBlockMapping,
        biome_tints: &BiomeTintResolver,
    ) -> MeshBatch {
        let mut batch = if use_greedy {
            build_chunk_mesh_greedy(
                self,
                self.center_key.0,
                self.center_key.1,
                leaf_depth_layer_faces,
                voxel_ao_enabled,
                voxel_ao_strength,
                voxel_ao_cutout,
                barrier_billboard,
                vanilla_bake,
                texture_mapping,
                biome_tints,
            )
        } else {
            build_chunk_mesh_culled(
                self,
                self.center_key.0,
                self.center_key.1,
                leaf_depth_layer_faces,
                voxel_ao_enabled,
                voxel_ao_strength,
                voxel_ao_cutout,
                barrier_billboard,
                vanilla_bake,
                texture_mapping,
                biome_tints,
            )
        };
        batch.occlusion = build_chunk_occlusion_data(self, self.center_key.0, self.center_key.1);
        batch
    }
}

pub struct MeshBatch {
    pub opaque: MeshData,
    pub cutout: MeshData,
    pub cutout_culled: MeshData,
    pub transparent: MeshData,
    pub occlusion: ChunkOcclusionData,
}

impl MeshBatch {
    pub fn data_for(&mut self, block_id: u16) -> &mut MeshData {
        match render_group_for_block(block_id) {
            MaterialGroup::Opaque => &mut self.opaque,
            MaterialGroup::Cutout => &mut self.cutout,
            MaterialGroup::CutoutCulled => &mut self.cutout_culled,
            MaterialGroup::Transparent => &mut self.transparent,
        }
    }
}

impl Default for MeshBatch {
    fn default() -> Self {
        Self {
            opaque: MeshData::empty(),
            cutout: MeshData::empty(),
            cutout_culled: MeshData::empty(),
            transparent: MeshData::empty(),
            occlusion: ChunkOcclusionData::default(),
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ChunkFace {
    NegX = 0,
    PosX = 1,
    NegY = 2,
    PosY = 3,
    NegZ = 4,
    PosZ = 5,
}

impl ChunkFace {
    pub const ALL: [Self; 6] = [
        Self::NegX,
        Self::PosX,
        Self::NegY,
        Self::PosY,
        Self::NegZ,
        Self::PosZ,
    ];

    pub const fn index(self) -> usize {
        self as usize
    }

    pub const fn bit(self) -> u8 {
        1u8 << self.index()
    }

    pub const fn opposite(self) -> Self {
        match self {
            Self::NegX => Self::PosX,
            Self::PosX => Self::NegX,
            Self::NegY => Self::PosY,
            Self::PosY => Self::NegY,
            Self::NegZ => Self::PosZ,
            Self::PosZ => Self::NegZ,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ChunkOcclusionData {
    pub face_open_mask: u8,
    pub face_connections: [u8; 6],
}

impl ChunkOcclusionData {
    pub fn fully_open() -> Self {
        Self {
            face_open_mask: 0b00_111111,
            face_connections: [0b00_111111; 6],
        }
    }

    pub fn is_face_open(self, face: ChunkFace) -> bool {
        (self.face_open_mask & face.bit()) != 0
    }

    pub fn is_connected(self, in_face: ChunkFace, out_face: ChunkFace) -> bool {
        (self.face_connections[in_face.index()] & out_face.bit()) != 0
    }
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum MaterialGroup {
    Opaque,
    Cutout,
    CutoutCulled,
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
    pub cutout_material: Handle<ChunkAtlasMaterial>,
    pub cutout_culled_material: Handle<ChunkAtlasMaterial>,
    pub transparent_material: Handle<ChunkAtlasMaterial>,
    pub atlas: Handle<Image>,
    pub skybox_texture: Handle<Image>,
    pub texture_mapping: Arc<AtlasBlockMapping>,
    pub biome_tints: Arc<BiomeTintResolver>,
    pub grass_overlay_info: Vec4,
}
