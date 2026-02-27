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
use crate::lighting::{lighting_uniform_for_mode, uses_shadowed_pbr_path};

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
    pub water_effects: Vec4,
    pub water_controls: Vec4,
    pub water_extra: Vec4,
    pub ssr_params: Vec4,
    pub debug_flags: Vec4,
    pub grass_overlay_info: Vec4,
    pub reflection_view_proj: Mat4,
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
    ChunkData(ChunkData),
    BlockUpdate(BlockUpdate),
}

#[derive(Resource, Default)]
pub struct ChunkUpdateQueue(pub Vec<WorldUpdate>);

#[derive(Resource, Default)]
pub struct ChunkRenderState {
    pub entries: HashMap<(i32, i32), ChunkEntry>,
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
            if self.sections[idx].is_none() {
                self.sections[idx] = Some(vec![0u16; 4096]);
            }
            if self.block_light_sections[idx].is_none() {
                self.block_light_sections[idx] = Some(vec![0u8; 4096]);
            }
            if self.sky_light_sections[idx].is_none() {
                self.sky_light_sections[idx] = Some(vec![15u8; 4096]);
            }
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

impl FromWorld for ChunkRenderAssets {
    fn from_world(world: &mut World) -> Self {
        let settings = world
            .get_resource::<crate::debug::RenderDebugSettings>()
            .cloned()
            .unwrap_or_default();
        let (mut atlas_image, texture_mapping, biome_tints) = load_or_build_atlas();
        let mut sampler = ImageSamplerDescriptor::nearest();
        sampler.address_mode_u = ImageAddressMode::ClampToEdge;
        sampler.address_mode_v = ImageAddressMode::ClampToEdge;
        sampler.address_mode_w = ImageAddressMode::ClampToEdge;
        // Keep atlas texels stable across quality/pipeline switches.
        // Mip filtering on cutout alpha introduces gray halos on transparent texels.
        sampler.mipmap_filter = bevy::image::ImageFilterMode::Nearest;
        sampler.lod_min_clamp = 0.0;
        sampler.lod_max_clamp = 0.0;
        atlas_image.sampler = ImageSampler::Descriptor(sampler);
        let atlas_handle = {
            let mut images = world.resource_mut::<Assets<Image>>();
            images.add(atlas_image)
        };
        let skybox_texture = world.resource::<AssetServer>().load("skybox.ktx2");
        let mut materials = world.resource_mut::<Assets<ChunkAtlasMaterial>>();
        let use_shadowed_pbr = uses_shadowed_pbr_path(&settings);
        // Alpha-mask cutout keeps foliage/plant depth ordering stable while
        // clipping transparent texels in the prepass.
        let cutout_alpha_mode = AlphaMode::Mask(0.5);
        let grass_side_origin = texture_mapping
            .texture_index_by_name("grass_side.png")
            .map(atlas_tile_origin)
            .unwrap_or([f32::NAN, f32::NAN]);
        let grass_overlay_origin = texture_mapping
            .texture_index_by_name("grass_side_overlay.png")
            .map(atlas_tile_origin)
            .unwrap_or([f32::NAN, f32::NAN]);
        let grass_overlay_info = Vec4::new(
            grass_side_origin[0],
            grass_side_origin[1],
            grass_overlay_origin[0],
            grass_overlay_origin[1],
        );

        let make_lighting = |pass_mode: f32| {
            let mut u = lighting_uniform_for_mode(&settings, pass_mode);
            u.grass_overlay_info = grass_overlay_info;
            u
        };

        let opaque_material = materials.add(ChunkAtlasMaterial {
            base: StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: None,
                metallic: 0.0,
                reflectance: 0.0,
                perceptual_roughness: 1.0,
                opaque_render_method: OpaqueRendererMethod::Forward,
                unlit: !use_shadowed_pbr,
                ..default()
            },
            extension: AtlasTextureExtension {
                atlas: atlas_handle.clone(),
                skybox: skybox_texture.clone(),
                lighting: make_lighting(0.0),
            },
        });
        let transparent_material = materials.add(ChunkAtlasMaterial {
            base: StandardMaterial {
                base_color: Color::srgba(1.0, 1.0, 1.0, 0.8),
                base_color_texture: None,
                metallic: 0.0,
                reflectance: 0.0,
                perceptual_roughness: 1.0,
                alpha_mode: AlphaMode::Blend,
                cull_mode: None,
                opaque_render_method: OpaqueRendererMethod::Forward,
                unlit: !use_shadowed_pbr,
                ..default()
            },
            extension: AtlasTextureExtension {
                atlas: atlas_handle.clone(),
                skybox: skybox_texture.clone(),
                lighting: make_lighting(1.0),
            },
        });
        let cutout_material = materials.add(ChunkAtlasMaterial {
            base: StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: None,
                metallic: 0.0,
                reflectance: 0.0,
                perceptual_roughness: 1.0,
                // Cutout uses binary discard in shader and alpha-mask pipeline.
                alpha_mode: cutout_alpha_mode,
                cull_mode: None,
                opaque_render_method: OpaqueRendererMethod::Forward,
                unlit: false,
                ..default()
            },
            extension: AtlasTextureExtension {
                atlas: atlas_handle.clone(),
                skybox: skybox_texture.clone(),
                lighting: make_lighting(2.0),
            },
        });
        let cutout_culled_material = materials.add(ChunkAtlasMaterial {
            base: StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: None,
                metallic: 0.0,
                reflectance: 0.0,
                perceptual_roughness: 1.0,
                // Same as cutout_material, but with backface culling.
                alpha_mode: cutout_alpha_mode,
                cull_mode: Some(bevy::render::render_resource::Face::Back),
                opaque_render_method: OpaqueRendererMethod::Forward,
                unlit: false,
                ..default()
            },
            extension: AtlasTextureExtension {
                atlas: atlas_handle.clone(),
                skybox: skybox_texture.clone(),
                lighting: make_lighting(2.0),
            },
        });

        Self {
            opaque_material,
            cutout_material,
            cutout_culled_material,
            transparent_material,
            atlas: atlas_handle,
            skybox_texture,
            texture_mapping,
            biome_tints,
            grass_overlay_info,
        }
    }
}

fn assets_root() -> PathBuf {
    ruststone_assets_root()
}

fn texture_root_path() -> PathBuf {
    assets_root().join(TEXTURE_BASE)
}

fn load_or_build_atlas() -> (Image, Arc<AtlasBlockMapping>, Arc<BiomeTintResolver>) {
    let textures_root = texture_root_path();
    let mut texture_names = collect_texture_names(&textures_root);
    texture_names.sort();
    texture_names.dedup();
    if texture_names.is_empty() {
        texture_names.push("missing_texture.png".to_string());
    } else if !texture_names
        .iter()
        .any(|name| name == "missing_texture.png")
    {
        texture_names.insert(0, "missing_texture.png".to_string());
    }
    if texture_names.len() > ATLAS_TILE_CAPACITY {
        warn!(
            "Block texture atlas overflow: {} textures but capacity is {}",
            texture_names.len(),
            ATLAS_TILE_CAPACITY
        );
        texture_names.truncate(ATLAS_TILE_CAPACITY);
    }

    let mut name_to_index = HashMap::with_capacity(texture_names.len());
    for (idx, name) in texture_names.iter().enumerate() {
        name_to_index.insert(name.clone(), idx as u16);
    }

    let mut tile_size = None;
    let mut atlas = None::<ImageBuffer<Rgba<u8>, Vec<u8>>>;

    for (idx, texture_name) in texture_names.iter().enumerate() {
        let texture_path = textures_root.join(texture_name);
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
        let mut rgba = rgba;
        apply_default_foliage_tint(texture_name, &mut rgba);
        normalize_overlay_mask_texture(texture_name, &mut rgba);
        force_opaque_texture_alpha(texture_name, &mut rgba);

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
        // Copy texels verbatim into the atlas.
        // `overlay` alpha-composites against the destination and can darken
        // partially transparent texels, which breaks cutout-style block textures.
        imageops::replace(atlas_buf, &rgba, x.into(), y.into());
    }

    let atlas = atlas.unwrap_or_else(|| {
        ImageBuffer::from_pixel(ATLAS_COLUMNS * 16, ATLAS_ROWS * 16, Rgba([0, 0, 0, 0]))
    });
    dump_atlas_debug_images(&atlas);
    let mut model_resolver = BlockModelResolver::new(default_model_roots());
    let mapping = Arc::new(build_block_texture_mapping(
        &name_to_index,
        Some(&mut model_resolver),
    ));
    let biome_tints = Arc::new(BiomeTintResolver::load(&texturepack_minecraft_root()));
    (
        bevy_image_from_rgba(DynamicImage::ImageRgba8(atlas)),
        mapping,
        biome_tints,
    )
}

fn apply_default_foliage_tint(texture_name: &str, img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>) {
    // Fallback tinting path for grayscale foliage textures.
    // This keeps cutout foliage readable even when runtime biome tinting is unavailable.
    let tint = if is_grass_tinted_texture(texture_name) {
        [0x7f_u8, 0xb2_u8, 0x38_u8]
    } else if is_foliage_tinted_texture(texture_name) {
        [0x48_u8, 0xb5_u8, 0x18_u8]
    } else {
        return;
    };
    for p in img.pixels_mut() {
        if p.0[3] == 0 {
            continue;
        }
        p.0[0] = ((u16::from(p.0[0]) * u16::from(tint[0])) / 255) as u8;
        p.0[1] = ((u16::from(p.0[1]) * u16::from(tint[1])) / 255) as u8;
        p.0[2] = ((u16::from(p.0[2]) * u16::from(tint[2])) / 255) as u8;
    }
}

fn normalize_overlay_mask_texture(texture_name: &str, img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>) {
    if !texture_name.ends_with("_overlay.png") {
        return;
    }
    // Some packs encode overlay mask in RGB while alpha is fully opaque.
    // Convert to canonical mask: white RGB + mask alpha.
    for p in img.pixels_mut() {
        let [r, g, b, a] = p.0;
        let luma = ((u16::from(r) + u16::from(g) + u16::from(b)) / 3) as u8;
        let mask = if a == 255 { luma } else { a };
        p.0 = [255, 255, 255, mask];
    }
}

fn force_opaque_texture_alpha(texture_name: &str, img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>) {
    // Keep regular ice fully opaque to avoid transparent-pass ordering/flicker issues.
    // This matches the requested visual style for now.
    if texture_name != "ice.png" {
        return;
    }
    for p in img.pixels_mut() {
        p.0[3] = 255;
    }
}

fn is_grass_tinted_texture(name: &str) -> bool {
    matches!(
        name,
        "tallgrass.png"
            | "fern.png"
            | "double_plant_grass_bottom.png"
            | "double_plant_grass_top.png"
            | "double_plant_fern_bottom.png"
            | "double_plant_fern_top.png"
            | "reeds.png"
    )
}

fn is_foliage_tinted_texture(name: &str) -> bool {
    name.starts_with("leaves_") || matches!(name, "vine.png" | "waterlily.png")
}

fn collect_texture_names(textures_root: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(textures_root) else {
        return out;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
        {
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                out.push(name.to_string());
            }
        }
    }
    out
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

fn dump_atlas_debug_images(atlas: &ImageBuffer<Rgba<u8>, Vec<u8>>) {
    let out_dir = assets_root().join("debug");
    if fs::create_dir_all(&out_dir).is_err() {
        return;
    }

    let atlas_path = out_dir.join("atlas_dump.png");
    let _ = DynamicImage::ImageRgba8(atlas.clone()).save(&atlas_path);

    let mut alpha_img =
        ImageBuffer::from_pixel(atlas.width(), atlas.height(), Rgba([0, 0, 0, 255]));
    for (src, dst) in atlas.pixels().zip(alpha_img.pixels_mut()) {
        let a = src.0[3];
        *dst = Rgba([a, a, a, 255]);
    }
    let alpha_path = out_dir.join("atlas_alpha.png");
    let _ = DynamicImage::ImageRgba8(alpha_img).save(&alpha_path);
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
    let MeshData {
        positions,
        normals,
        mut uvs,
        uvs_b,
        colors,
        indices,
        ..
    } = data;

    // Pack tile cell into UV0 so shader path remains stable even if UV1 is omitted
    // by some internal pipeline variants.
    for (uv, tile_origin) in uvs.iter_mut().zip(uvs_b.iter()) {
        let col = (tile_origin[0] * ATLAS_COLUMNS as f32).round();
        let row = (tile_origin[1] * ATLAS_ROWS as f32).round();
        uv[0] += col * ATLAS_UV_PACK_SCALE;
        uv[1] += row * ATLAS_UV_PACK_SCALE;
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, uvs_b);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
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
        column.set_section(
            section.y,
            section.blocks,
            section.block_light,
            section.sky_light,
        );
    }
}

pub fn apply_block_update(store: &mut ChunkStore, update: BlockUpdate) -> Vec<(i32, i32)> {
    if !(0..WORLD_HEIGHT).contains(&update.y) {
        return Vec::new();
    }

    let chunk_x = update.x.div_euclid(CHUNK_SIZE);
    let chunk_z = update.z.div_euclid(CHUNK_SIZE);
    let local_x = update.x.rem_euclid(CHUNK_SIZE) as usize;
    let local_z = update.z.rem_euclid(CHUNK_SIZE) as usize;

    let column = store
        .chunks
        .entry((chunk_x, chunk_z))
        .or_insert_with(ChunkColumn::new);
    column.set_block(local_x, update.y, local_z, update.block_id);

    let mut touched = vec![(chunk_x, chunk_z)];
    if local_x == 0 {
        touched.push((chunk_x - 1, chunk_z));
    }
    if local_x == (CHUNK_SIZE as usize - 1) {
        touched.push((chunk_x + 1, chunk_z));
    }
    if local_z == 0 {
        touched.push((chunk_x, chunk_z - 1));
    }
    if local_z == (CHUNK_SIZE as usize - 1) {
        touched.push((chunk_x, chunk_z + 1));
    }
    touched
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

fn build_chunk_occlusion_data(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
) -> ChunkOcclusionData {
    let Some(column) = snapshot.columns.get(&(chunk_x, chunk_z)) else {
        return ChunkOcclusionData::fully_open();
    };

    let mut out = ChunkOcclusionData::default();
    let mut visited = vec![false; (CHUNK_SIZE * WORLD_HEIGHT * CHUNK_SIZE) as usize];
    let mut queue = VecDeque::new();

    let idx_of = |x: i32, y: i32, z: i32| -> usize {
        ((y * CHUNK_SIZE * CHUNK_SIZE) + (z * CHUNK_SIZE) + x) as usize
    };
    let local_block = |x: i32, y: i32, z: i32| -> u16 {
        let section_idx = (y / SECTION_HEIGHT) as usize;
        let local_y = (y % SECTION_HEIGHT) as usize;
        column
            .sections
            .get(section_idx)
            .and_then(|section| section.as_ref())
            .map(|section| {
                let block_idx = local_y * 16 * 16 + z as usize * 16 + x as usize;
                section[block_idx]
            })
            .unwrap_or(0)
    };
    let is_local_passable = |x: i32, y: i32, z: i32| -> bool {
        let block = local_block(x, y, z);
        !is_occluding_block(block)
    };
    let face_at = |x: i32, y: i32, z: i32| -> u8 {
        let mut mask = 0u8;
        if x == 0 {
            mask |= ChunkFace::NegX.bit();
        }
        if x == CHUNK_SIZE - 1 {
            mask |= ChunkFace::PosX.bit();
        }
        if y == 0 {
            mask |= ChunkFace::NegY.bit();
        }
        if y == WORLD_HEIGHT - 1 {
            mask |= ChunkFace::PosY.bit();
        }
        if z == 0 {
            mask |= ChunkFace::NegZ.bit();
        }
        if z == CHUNK_SIZE - 1 {
            mask |= ChunkFace::PosZ.bit();
        }
        mask
    };
    let mut connect_component_faces = |component_faces: u8| {
        if component_faces == 0 {
            return;
        }
        out.face_open_mask |= component_faces;
        for face in ChunkFace::ALL {
            if (component_faces & face.bit()) == 0 {
                continue;
            }
            out.face_connections[face.index()] |= component_faces;
        }
    };

    for y in 0..WORLD_HEIGHT {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let boundary_mask = face_at(x, y, z);
                if boundary_mask == 0 {
                    continue;
                }
                if !is_local_passable(x, y, z) {
                    continue;
                }
                let seed_idx = idx_of(x, y, z);
                if visited[seed_idx] {
                    continue;
                }

                visited[seed_idx] = true;
                queue.push_back((x, y, z));
                let mut component_faces = 0u8;

                while let Some((cx, cy, cz)) = queue.pop_front() {
                    component_faces |= face_at(cx, cy, cz);
                    for (dx, dy, dz) in [
                        (-1, 0, 0),
                        (1, 0, 0),
                        (0, -1, 0),
                        (0, 1, 0),
                        (0, 0, -1),
                        (0, 0, 1),
                    ] {
                        let nx = cx + dx;
                        let ny = cy + dy;
                        let nz = cz + dz;
                        if !(0..CHUNK_SIZE).contains(&nx)
                            || !(0..WORLD_HEIGHT).contains(&ny)
                            || !(0..CHUNK_SIZE).contains(&nz)
                        {
                            continue;
                        }
                        let nidx = idx_of(nx, ny, nz);
                        if visited[nidx] || !is_local_passable(nx, ny, nz) {
                            continue;
                        }
                        visited[nidx] = true;
                        queue.push_back((nx, ny, nz));
                    }
                }

                connect_component_faces(component_faces);
            }
        }
    }

    out
}

fn build_chunk_mesh_culled(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    leaf_depth_layer_faces: bool,
    voxel_ao_enabled: bool,
    voxel_ao_strength: f32,
    voxel_ao_cutout: bool,
    texture_mapping: &AtlasBlockMapping,
    biome_tints: &BiomeTintResolver,
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
                    if block_type(block_id) == 0 {
                        continue;
                    }

                    let tint = biome_tint_at(snapshot, chunk_x, chunk_z, x, z, biome_tints);
                    if is_custom_block(block_id) {
                        add_custom_block(
                            &mut batch,
                            snapshot,
                            texture_mapping,
                            biome_tints,
                            chunk_x,
                            chunk_z,
                            x,
                            base_y + y,
                            z,
                            block_id,
                            tint,
                        );
                        continue;
                    }

                    add_block_faces(
                        &mut batch,
                        snapshot,
                        texture_mapping,
                        biome_tints,
                        leaf_depth_layer_faces,
                        voxel_ao_enabled,
                        voxel_ao_strength,
                        voxel_ao_cutout,
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
    texture_index: u16,
    block_id: u16,
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
    leaf_depth_layer_faces: bool,
    voxel_ao_enabled: bool,
    voxel_ao_strength: f32,
    voxel_ao_cutout: bool,
    texture_mapping: &AtlasBlockMapping,
    biome_tints: &BiomeTintResolver,
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

        // Cross-model blocks (flowers/grass/reeds/crops) are emitted as explicit
        // crossed quads and skipped from greedy full-cube meshing.
        for y in 0..SECTION_HEIGHT {
            for z in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let idx = (y * CHUNK_SIZE * CHUNK_SIZE + z * CHUNK_SIZE + x) as usize;
                    let block_id = section_blocks[idx];
                    if block_type(block_id) == 0 || !is_custom_block(block_id) {
                        continue;
                    }
                    let tint = biome_tint_at(snapshot, chunk_x, chunk_z, x, z, biome_tints);
                    add_custom_block(
                        &mut batch,
                        snapshot,
                        texture_mapping,
                        biome_tints,
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
                        if block_type(block_id) == 0 {
                            continue;
                        }
                        if is_custom_block(block_id) {
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
                        if face_is_occluded(block_id, neighbor, leaf_depth_layer_faces) {
                            continue;
                        }

                        let texture_index = texture_mapping.texture_index_for_state(block_id, face);
                        let biome_id = biome_at(snapshot, chunk_x, chunk_z, x, z);
                        let tint_key = if !matches!(
                            classify_tint(block_id, None),
                            TintClass::None | TintClass::FoliageFixed(_)
                        ) {
                            biome_id
                        } else {
                            0
                        };
                        let key = GreedyKey {
                            texture_index,
                            block_id,
                            tint_key,
                        };

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
                        let tint = biome_tints.tint_for_biome(key.tint_key);
                        add_greedy_quad(
                            &mut batch,
                            snapshot,
                            chunk_x,
                            chunk_z,
                            face,
                            axis as i32,
                            base_y,
                            quad,
                            key.texture_index,
                            key.block_id,
                            tint,
                            voxel_ao_enabled,
                            voxel_ao_strength,
                            voxel_ao_cutout,
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
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    face: Face,
    axis: i32,
    base_y: i32,
    quad: GreedyQuad,
    texture_index: u16,
    block_id: u16,
    tint: BiomeTint,
    voxel_ao_enabled: bool,
    voxel_ao_strength: f32,
    voxel_ao_cutout: bool,
) {
    let data = batch.data_for(block_id);
    let base_index = data.positions.len() as u32;
    let tile_origin = atlas_tile_origin(texture_index);

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

    let base_uvs = uv_for_texture();
    for uv in base_uvs {
        data.uvs
            .push([uv[0] * quad.w as f32, uv[1] * quad.h as f32]);
        data.uvs_b.push(tile_origin);
    }
    let base_color = tint_color_untargeted(block_id, tint);
    let shades = greedy_face_corner_shades(
        snapshot,
        chunk_x,
        chunk_z,
        face,
        axis,
        base_y,
        &quad,
        block_id,
        voxel_ao_enabled,
        voxel_ao_strength,
        voxel_ao_cutout,
    );
    for shade in shades {
        if is_grass_side_face(block_id, face) {
            data.colors
                .push([base_color[0], base_color[1], base_color[2], shade]);
        } else {
            data.colors.push([
                base_color[0] * shade,
                base_color[1] * shade,
                base_color[2] * shade,
                base_color[3],
            ]);
        }
    }

    let use_alt_diag = (shades[0] + shades[2]) > (shades[1] + shades[3]);
    if use_alt_diag {
        data.indices.extend_from_slice(&[
            base_index,
            base_index + 3,
            base_index + 1,
            base_index + 1,
            base_index + 3,
            base_index + 2,
        ]);
    } else {
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
    texture_mapping: &AtlasBlockMapping,
    biome_tints: &BiomeTintResolver,
    leaf_depth_layer_faces: bool,
    voxel_ao_enabled: bool,
    voxel_ao_strength: f32,
    voxel_ao_cutout: bool,
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
        if face_is_occluded(block_id, neighbor, leaf_depth_layer_faces) {
            continue;
        }

        let texture_index = texture_mapping.texture_index_for_state(block_id, face);
        let data = batch.data_for(block_id);
        let base_index = data.positions.len() as u32;
        for vert in verts {
            data.push_pos([vert[0] + x as f32, vert[1] + y as f32, vert[2] + z as f32]);
            data.normals.push(normal);
        }
        let uvs = uv_for_texture();
        data.uvs.extend_from_slice(&uvs);
        let tile_origin = atlas_tile_origin(texture_index);
        data.uvs_b.extend_from_slice(&[tile_origin; 4]);
        let base_color = tint_color(
            block_id,
            tint,
            snapshot,
            chunk_x,
            chunk_z,
            x,
            y,
            z,
            biome_tints,
        );
        for vert in verts {
            let shade = compute_vertex_shade(
                snapshot,
                chunk_x,
                chunk_z,
                x,
                y,
                z,
                face,
                vert,
                block_id,
                voxel_ao_enabled,
                voxel_ao_strength,
                voxel_ao_cutout,
            );
            if is_grass_side_face(block_id, face) {
                data.colors
                    .push([base_color[0], base_color[1], base_color[2], shade]);
            } else {
                data.colors.push([
                    base_color[0] * shade,
                    base_color[1] * shade,
                    base_color[2] * shade,
                    base_color[3],
                ]);
            }
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

fn is_grass_side_face(block_state: u16, face: Face) -> bool {
    block_type(block_state) == 2 && !matches!(face, Face::PosY | Face::NegY)
}

#[allow(clippy::too_many_arguments)]
fn tint_color(
    block_id: u16,
    tint: BiomeTint,
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    biome_tints: &BiomeTintResolver,
) -> [f32; 4] {
    let below = if block_type(block_id) == 175 {
        Some(block_at(snapshot, chunk_x, chunk_z, x, y - 1, z))
    } else {
        None
    };
    match classify_tint(block_id, below) {
        TintClass::Grass => tint.grass,
        TintClass::Foliage => tint.foliage,
        TintClass::Water => [tint.water[0], tint.water[1], tint.water[2], 0.5],
        TintClass::FoliageFixed(rgb) => {
            let r = ((rgb >> 16) & 0xFF) as f32 / 255.0;
            let g = ((rgb >> 8) & 0xFF) as f32 / 255.0;
            let b = (rgb & 0xFF) as f32 / 255.0;
            [r, g, b, 1.0]
        }
        TintClass::None => {
            let _ = biome_tints;
            [1.0, 1.0, 1.0, 1.0]
        }
    }
}

fn tint_color_untargeted(block_id: u16, tint: BiomeTint) -> [f32; 4] {
    match classify_tint(block_id, None) {
        TintClass::Grass => tint.grass,
        TintClass::Foliage => tint.foliage,
        TintClass::Water => [tint.water[0], tint.water[1], tint.water[2], 0.5],
        TintClass::FoliageFixed(rgb) => [
            ((rgb >> 16) & 0xFF) as f32 / 255.0,
            ((rgb >> 8) & 0xFF) as f32 / 255.0,
            (rgb & 0xFF) as f32 / 255.0,
            1.0,
        ],
        TintClass::None => [1.0, 1.0, 1.0, 1.0],
    }
}

fn add_cross_plant(
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
    let texture_index = texture_mapping.texture_index_for_state(block_id, Face::PosZ);
    let tile_origin = atlas_tile_origin(texture_index);
    let uvs = uv_for_texture();
    let mut color = tint_color(
        block_id,
        tint,
        snapshot,
        chunk_x,
        chunk_z,
        x,
        y,
        z,
        biome_tints,
    );
    if let Some(tint_rgb) =
        cross_vegetation_biome_tint(block_id, snapshot, chunk_x, chunk_z, x, y, z, tint)
    {
        color[0] = tint_rgb[0];
        color[1] = tint_rgb[1];
        color[2] = tint_rgb[2];
    }
    let shade = if should_apply_prebaked_shade(block_id) {
        block_light_factor(snapshot, chunk_x, chunk_z, x, y, z)
    } else {
        1.0
    };
    color[0] *= shade;
    color[1] *= shade;
    color[2] *= shade;
    let data = batch.data_for(block_id);

    let x0 = x as f32;
    let y0 = y as f32;
    let z0 = z as f32;

    // Cross-plant quads use gently lifted normals so they don't appear unnaturally
    // dark from top/side lighting compared to nearby terrain.
    let cross_normal_lift = |n: Vec3| -> [f32; 3] {
        let lifted = Vec3::new(n.x, 0.38, n.z).normalize_or_zero();
        [lifted.x, lifted.y, lifted.z]
    };

    // Plane A: (0,0,0) -> (1,1,1)
    let normal_a = cross_normal_lift(Vec3::new(1.0, 0.0, 1.0));
    let a = [
        [x0 + 0.0, y0 + 0.0, z0 + 0.0],
        [x0 + 1.0, y0 + 0.0, z0 + 1.0],
        [x0 + 1.0, y0 + 1.0, z0 + 1.0],
        [x0 + 0.0, y0 + 1.0, z0 + 0.0],
    ];
    add_double_sided_quad(data, a, normal_a, uvs, tile_origin, color);

    // Plane B: (1,0,0) -> (0,1,1)
    let normal_b = cross_normal_lift(Vec3::new(-1.0, 0.0, 1.0));
    let b = [
        [x0 + 1.0, y0 + 0.0, z0 + 0.0],
        [x0 + 0.0, y0 + 0.0, z0 + 1.0],
        [x0 + 0.0, y0 + 1.0, z0 + 1.0],
        [x0 + 1.0, y0 + 1.0, z0 + 0.0],
    ];
    add_double_sided_quad(data, b, normal_b, uvs, tile_origin, color);
}

fn cross_vegetation_biome_tint(
    block_state: u16,
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    tint: BiomeTint,
) -> Option<[f32; 3]> {
    let id = block_type(block_state);
    match id {
        // Saplings
        6 => Some([tint.foliage[0], tint.foliage[1], tint.foliage[2]]),
        // Tallgrass / fern
        31 => Some([tint.grass[0], tint.grass[1], tint.grass[2]]),
        // Reeds
        83 => Some([tint.grass[0], tint.grass[1], tint.grass[2]]),
        // Double grass / fern (upper half resolves from the lower block metadata)
        175 => {
            let meta = block_meta(block_state);
            let lower_meta = if (meta & 0x8) != 0 {
                block_meta(block_at(snapshot, chunk_x, chunk_z, x, y - 1, z))
            } else {
                meta
            };
            if matches!(lower_meta & 0x7, 2 | 3) {
                Some([tint.grass[0], tint.grass[1], tint.grass[2]])
            } else {
                None
            }
        }
        _ => None,
    }
}

fn add_double_sided_quad(
    data: &mut MeshData,
    verts: [[f32; 3]; 4],
    normal: [f32; 3],
    uvs: [[f32; 2]; 4],
    tile_origin: [f32; 2],
    color: [f32; 4],
) {
    let base = data.positions.len() as u32;
    for i in 0..4 {
        data.push_pos(verts[i]);
        data.normals.push(normal);
        data.uvs.push(uvs[i]);
        data.uvs_b.push(tile_origin);
        data.colors.push(color);
    }
    data.indices
        .extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);

    let back_base = data.positions.len() as u32;
    for i in 0..4 {
        data.push_pos(verts[i]);
        // Keep the same normal on both sides to avoid one face always receiving
        // inverted lighting on billboard-style vegetation quads.
        data.normals.push(normal);
        data.uvs.push(uvs[i]);
        data.uvs_b.push(tile_origin);
        data.colors.push(color);
    }
    data.indices.extend_from_slice(&[
        back_base,
        back_base + 1,
        back_base + 2,
        back_base,
        back_base + 2,
        back_base + 3,
    ]);
}

fn add_custom_block(
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
    match block_model_kind(block_type(block_id)) {
        BlockModelKind::Cross => add_cross_plant(
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
        BlockModelKind::TorchLike => add_cross_plant(
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
        BlockModelKind::Slab => add_box(
            batch,
            Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
            texture_mapping,
            biome_tints,
            x,
            y,
            z,
            if (block_meta(block_id) & 0x8) != 0 {
                [0.0, 0.5, 0.0]
            } else {
                [0.0, 0.0, 0.0]
            },
            if (block_meta(block_id) & 0x8) != 0 {
                [1.0, 1.0, 1.0]
            } else {
                [1.0, 0.5, 1.0]
            },
            block_id,
            tint,
        ),
        BlockModelKind::Stairs => {
            let meta = block_meta(block_id);
            let top = (meta & 0x4) != 0;
            let facing = meta & 0x3;

            // Base slab half.
            add_box(
                batch,
                Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
                texture_mapping,
                biome_tints,
                x,
                y,
                z,
                if top {
                    [0.0, 0.5, 0.0]
                } else {
                    [0.0, 0.0, 0.0]
                },
                if top {
                    [1.0, 1.0, 1.0]
                } else {
                    [1.0, 0.5, 1.0]
                },
                block_id,
                tint,
            );

            // Vertical half matching the stair facing.
            let (min_x, max_x, min_z, max_z) = match facing {
                0 => (0.5, 1.0, 0.0, 1.0), // east
                1 => (0.0, 0.5, 0.0, 1.0), // west
                2 => (0.0, 1.0, 0.5, 1.0), // south
                _ => (0.0, 1.0, 0.0, 0.5), // north
            };
            add_box(
                batch,
                Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
                texture_mapping,
                biome_tints,
                x,
                y,
                z,
                if top {
                    [min_x, 0.0, min_z]
                } else {
                    [min_x, 0.5, min_z]
                },
                if top {
                    [max_x, 0.5, max_z]
                } else {
                    [max_x, 1.0, max_z]
                },
                block_id,
                tint,
            );
        }
        BlockModelKind::Fence => {
            let connect_east = fence_connects_to(block_at(snapshot, chunk_x, chunk_z, x + 1, y, z));
            let connect_west = fence_connects_to(block_at(snapshot, chunk_x, chunk_z, x - 1, y, z));
            let connect_south =
                fence_connects_to(block_at(snapshot, chunk_x, chunk_z, x, y, z + 1));
            let connect_north =
                fence_connects_to(block_at(snapshot, chunk_x, chunk_z, x, y, z - 1));
            add_box(
                batch,
                None,
                texture_mapping,
                biome_tints,
                x,
                y,
                z,
                [0.375, 0.0, 0.375],
                [0.625, 1.0, 0.625],
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
                    [0.4375, 0.375, 0.0],
                    [0.5625, 0.8125, 0.5],
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
                    [0.4375, 0.375, 0.5],
                    [0.5625, 0.8125, 1.0],
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
                    [0.0, 0.375, 0.4375],
                    [0.5, 0.8125, 0.5625],
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
                    [0.5, 0.375, 0.4375],
                    [1.0, 0.8125, 0.5625],
                    block_id,
                    tint,
                );
            }
        }
        BlockModelKind::Pane => {
            let connect_east = pane_connects_to(block_at(snapshot, chunk_x, chunk_z, x + 1, y, z));
            let connect_west = pane_connects_to(block_at(snapshot, chunk_x, chunk_z, x - 1, y, z));
            let connect_south = pane_connects_to(block_at(snapshot, chunk_x, chunk_z, x, y, z + 1));
            let connect_north = pane_connects_to(block_at(snapshot, chunk_x, chunk_z, x, y, z - 1));
            let has_x = connect_east || connect_west;
            let has_z = connect_north || connect_south;
            let add_center = !has_x || !has_z;

            if add_center {
                add_box(
                    batch,
                    None,
                    texture_mapping,
                    biome_tints,
                    x,
                    y,
                    z,
                    [0.4375, 0.0, 0.4375],
                    [0.5625, 1.0, 0.5625],
                    block_id,
                    tint,
                );
            }

            if has_z {
                if connect_north {
                    add_box(
                        batch,
                        None,
                        texture_mapping,
                        biome_tints,
                        x,
                        y,
                        z,
                        [0.4375, 0.0, 0.0],
                        [0.5625, 1.0, 0.5],
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
                        [0.4375, 0.0, 0.5],
                        [0.5625, 1.0, 1.0],
                        block_id,
                        tint,
                    );
                }
            }

            if has_x {
                if connect_west {
                    add_box(
                        batch,
                        None,
                        texture_mapping,
                        biome_tints,
                        x,
                        y,
                        z,
                        [0.0, 0.0, 0.4375],
                        [0.5, 1.0, 0.5625],
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
                        [0.5, 0.0, 0.4375],
                        [1.0, 1.0, 0.5625],
                        block_id,
                        tint,
                    );
                }
            }
        }
        BlockModelKind::Custom => {
            let id = block_type(block_id);
            match id {
                // Chests / trapped chests / ender chests.
                54 | 130 | 146 => add_box(
                    batch,
                    Some((snapshot, chunk_x, chunk_z, x, y, z, block_id)),
                    texture_mapping,
                    biome_tints,
                    x,
                    y,
                    z,
                    [1.0 / 16.0, 0.0, 1.0 / 16.0],
                    [15.0 / 16.0, 14.0 / 16.0, 15.0 / 16.0],
                    block_id,
                    tint,
                ),
                // Bed: low 9/16-height block.
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
                // Rails: thin top plate (including powered/detector/activator variants).
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
                // Doors: full-height thin panel, orientation/open from lower-half metadata.
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
                // Fence gates: central panel + side posts, rotates when open.
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
                // Cobblestone walls (meta 0/1 texture handled via texture mapping).
                139 => {
                    let connect_east =
                        wall_connects_to(block_at(snapshot, chunk_x, chunk_z, x + 1, y, z));
                    let connect_west =
                        wall_connects_to(block_at(snapshot, chunk_x, chunk_z, x - 1, y, z));
                    let connect_south =
                        wall_connects_to(block_at(snapshot, chunk_x, chunk_z, x, y, z + 1));
                    let connect_north =
                        wall_connects_to(block_at(snapshot, chunk_x, chunk_z, x, y, z - 1));
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
                _ => {}
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn add_box(
    batch: &mut MeshBatch,
    neighbor_ctx: Option<(&ChunkColumnSnapshot, i32, i32, i32, i32, i32, u16)>,
    texture_mapping: &AtlasBlockMapping,
    biome_tints: &BiomeTintResolver,
    x: i32,
    y: i32,
    z: i32,
    min: [f32; 3],
    max: [f32; 3],
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
                [max[0], min[1], min[2]],
                [max[0], min[1], max[2]],
                [max[0], max[1], max[2]],
                [max[0], max[1], min[2]],
            ],
            max[0] >= 1.0,
        ),
        (
            Face::NegX,
            -1,
            0,
            0,
            [-1.0, 0.0, 0.0],
            [
                [min[0], min[1], max[2]],
                [min[0], min[1], min[2]],
                [min[0], max[1], min[2]],
                [min[0], max[1], max[2]],
            ],
            min[0] <= 0.0,
        ),
        (
            Face::PosY,
            0,
            1,
            0,
            [0.0, 1.0, 0.0],
            [
                [min[0], max[1], min[2]],
                [max[0], max[1], min[2]],
                [max[0], max[1], max[2]],
                [min[0], max[1], max[2]],
            ],
            max[1] >= 1.0,
        ),
        (
            Face::NegY,
            0,
            -1,
            0,
            [0.0, -1.0, 0.0],
            [
                [min[0], min[1], max[2]],
                [max[0], min[1], max[2]],
                [max[0], min[1], min[2]],
                [min[0], min[1], min[2]],
            ],
            min[1] <= 0.0,
        ),
        (
            Face::PosZ,
            0,
            0,
            1,
            [0.0, 0.0, 1.0],
            [
                [max[0], min[1], max[2]],
                [min[0], min[1], max[2]],
                [min[0], max[1], max[2]],
                [max[0], max[1], max[2]],
            ],
            max[2] >= 1.0,
        ),
        (
            Face::NegZ,
            0,
            0,
            -1,
            [0.0, 0.0, -1.0],
            [
                [min[0], min[1], min[2]],
                [max[0], min[1], min[2]],
                [max[0], max[1], min[2]],
                [min[0], max[1], min[2]],
            ],
            min[2] <= 0.0,
        ),
    ];

    for (face, dx, dy, dz, normal, verts, boundary_face) in faces {
        if let Some((snapshot, chunk_x, chunk_z, bx, by, bz, block_id_for_cull)) = neighbor_ctx {
            if boundary_face {
                let neighbor = block_at(snapshot, chunk_x, chunk_z, bx + dx, by + dy, bz + dz);
                if face_is_occluded(block_id_for_cull, neighbor, true) {
                    continue;
                }
            }
        }

        let texture_index = texture_mapping.texture_index_for_state(block_id, face);
        let data = batch.data_for(block_id);
        let base_index = data.positions.len() as u32;
        for vert in verts {
            data.push_pos([vert[0] + x as f32, vert[1] + y as f32, vert[2] + z as f32]);
            data.normals.push(normal);
        }
        let uvs = uv_for_texture();
        data.uvs.extend_from_slice(&uvs);
        let tile_origin = atlas_tile_origin(texture_index);
        data.uvs_b.extend_from_slice(&[tile_origin; 4]);
        let mut color = if let Some((snapshot, chunk_x, chunk_z, bx, by, bz, _)) = neighbor_ctx {
            tint_color(
                block_id,
                tint,
                snapshot,
                chunk_x,
                chunk_z,
                bx,
                by,
                bz,
                biome_tints,
            )
        } else {
            tint_color_untargeted(block_id, tint)
        };
        if let Some((snapshot, chunk_x, chunk_z, bx, by, bz, _)) = neighbor_ctx {
            let shade = if should_apply_prebaked_shade(block_id) {
                face_light_factor(snapshot, chunk_x, chunk_z, bx, by, bz, face)
            } else {
                1.0
            };
            color[0] *= shade;
            color[1] *= shade;
            color[2] *= shade;
        }
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

fn is_custom_block(block_id: u16) -> bool {
    matches!(
        block_model_kind(block_type(block_id)),
        BlockModelKind::Cross
            | BlockModelKind::Slab
            | BlockModelKind::Stairs
            | BlockModelKind::Fence
            | BlockModelKind::Pane
            | BlockModelKind::TorchLike
            | BlockModelKind::Custom
    )
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

fn biome_tint_at(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    z: i32,
    resolver: &BiomeTintResolver,
) -> BiomeTint {
    let mut grass = [0.0f32; 3];
    let mut foliage = [0.0f32; 3];
    let mut water = [0.0f32; 3];
    let mut count = 0.0f32;

    for dz in -1..=1 {
        for dx in -1..=1 {
            let wx = x + dx;
            let wz = z + dz;
            let mut cx = chunk_x;
            let mut cz = chunk_z;
            let mut lx = wx;
            let mut lz = wz;
            if lx < 0 {
                cx -= 1;
                lx += 16;
            } else if lx >= 16 {
                cx += 1;
                lx -= 16;
            }
            if lz < 0 {
                cz -= 1;
                lz += 16;
            } else if lz >= 16 {
                cz += 1;
                lz -= 16;
            }
            let bt = resolver.tint_for_biome(biome_at(snapshot, cx, cz, lx, lz));
            grass[0] += bt.grass[0];
            grass[1] += bt.grass[1];
            grass[2] += bt.grass[2];
            foliage[0] += bt.foliage[0];
            foliage[1] += bt.foliage[1];
            foliage[2] += bt.foliage[2];
            water[0] += bt.water[0];
            water[1] += bt.water[1];
            water[2] += bt.water[2];
            count += 1.0;
        }
    }

    BiomeTint {
        grass: [grass[0] / count, grass[1] / count, grass[2] / count, 1.0],
        foliage: [
            foliage[0] / count,
            foliage[1] / count,
            foliage[2] / count,
            1.0,
        ],
        water: [water[0] / count, water[1] / count, water[2] / count, 1.0],
    }
}

fn resolve_chunk_coords(chunk_x: i32, chunk_z: i32, x: i32, z: i32) -> (i32, i32, i32, i32) {
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

    (target_chunk_x, target_chunk_z, local_x, local_z)
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

    let (target_chunk_x, target_chunk_z, local_x, local_z) =
        resolve_chunk_coords(chunk_x, chunk_z, x, z);

    let Some(column) = snapshot.columns.get(&(target_chunk_x, target_chunk_z)) else {
        // Neighbor chunk not loaded: treat as air so border faces get generated.
        return 0;
    };

    let section_index = (y / SECTION_HEIGHT) as usize;
    let local_y = (y % SECTION_HEIGHT) as usize;

    let Some(section) = column.sections.get(section_index).and_then(|v| v.as_ref()) else {
        // Unloaded section: treat as air for rendering purposes.
        return 0;
    };

    let idx = local_y * 16 * 16 + local_z as usize * 16 + local_x as usize;
    section[idx]
}

#[derive(Clone, Copy, Default)]
struct VoxelLight {
    block: u8,
    sky: u8,
}

fn light_at(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
) -> VoxelLight {
    if y < 0 || y >= WORLD_HEIGHT {
        return VoxelLight::default();
    }

    let (target_chunk_x, target_chunk_z, local_x, local_z) =
        resolve_chunk_coords(chunk_x, chunk_z, x, z);
    let Some(column) = snapshot.columns.get(&(target_chunk_x, target_chunk_z)) else {
        return VoxelLight::default();
    };
    let section_index = (y / SECTION_HEIGHT) as usize;
    let local_y = (y % SECTION_HEIGHT) as usize;
    let idx = local_y * 16 * 16 + local_z as usize * 16 + local_x as usize;

    let block = column
        .block_light_sections
        .get(section_index)
        .and_then(|v| v.as_ref())
        .and_then(|s| s.get(idx))
        .copied()
        .unwrap_or(0);
    let sky = column
        .sky_light_sections
        .get(section_index)
        .and_then(|v| v.as_ref())
        .and_then(|s| s.get(idx))
        .copied()
        .unwrap_or(0);
    VoxelLight { block, sky }
}

fn is_ao_occluder(block_state: u16) -> bool {
    let id = block_type(block_state);
    if id == 0 {
        return false;
    }
    if is_transparent_block(id) {
        return false;
    }
    !matches!(
        block_model_kind(id),
        BlockModelKind::Cross
            | BlockModelKind::Pane
            | BlockModelKind::TorchLike
            | BlockModelKind::Custom
    )
}

fn ao_factor(side1: bool, side2: bool, corner: bool) -> f32 {
    let level = if side1 && side2 {
        0
    } else {
        3 - (side1 as u8 + side2 as u8 + corner as u8)
    };
    match level {
        0 => 0.56,
        1 => 0.70,
        2 => 0.84,
        _ => 1.0,
    }
}

fn light_factor_from_level(level: f32) -> f32 {
    // Keep a minimum floor so caves are dark but not pure black.
    (0.18 + (level / 15.0) * 0.82).clamp(0.0, 1.0)
}

fn can_apply_vertex_shading(block_id: u16, voxel_ao_cutout: bool) -> bool {
    match render_group_for_block(block_id) {
        MaterialGroup::Opaque => true,
        MaterialGroup::Cutout | MaterialGroup::CutoutCulled => voxel_ao_cutout,
        MaterialGroup::Transparent => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn compute_vertex_shade(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    face: Face,
    vertex: [f32; 3],
    block_id: u16,
    voxel_ao_enabled: bool,
    voxel_ao_strength: f32,
    voxel_ao_cutout: bool,
) -> f32 {
    if !can_apply_vertex_shading(block_id, voxel_ao_cutout) {
        return 1.0;
    }
    let (ao, light) = face_vertex_light_ao(snapshot, chunk_x, chunk_z, x, y, z, face, vertex);
    if voxel_ao_enabled {
        let s = voxel_ao_strength.clamp(0.0, 1.0);
        light * (1.0 - s + ao * s)
    } else {
        light
    }
}

fn face_vertex_light_ao(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    face: Face,
    vertex: [f32; 3],
) -> (f32, f32) {
    let (nx, ny, nz, axis_a, axis_b) = match face {
        Face::PosX => (1, 0, 0, 1usize, 2usize), // y,z
        Face::NegX => (-1, 0, 0, 1usize, 2usize),
        Face::PosY => (0, 1, 0, 0usize, 2usize), // x,z
        Face::NegY => (0, -1, 0, 0usize, 2usize),
        Face::PosZ => (0, 0, 1, 0usize, 1usize), // x,y
        Face::NegZ => (0, 0, -1, 0usize, 1usize),
    };

    let signs = |coord: f32| if coord <= 0.0 { -1 } else { 1 };
    let mut delta = [0i32; 3];
    delta[axis_a] = signs(vertex[axis_a]);
    delta[axis_b] = signs(vertex[axis_b]);

    let base = (x + nx, y + ny, z + nz);
    let s1 = (base.0 + delta[0], base.1 + delta[1], base.2 + delta[2]);
    let mut side1 = [base.0, base.1, base.2];
    side1[axis_a] += delta[axis_a];
    let mut side2 = [base.0, base.1, base.2];
    side2[axis_b] += delta[axis_b];

    let occ_side1 = is_ao_occluder(block_at(
        snapshot, chunk_x, chunk_z, side1[0], side1[1], side1[2],
    ));
    let occ_side2 = is_ao_occluder(block_at(
        snapshot, chunk_x, chunk_z, side2[0], side2[1], side2[2],
    ));
    let occ_corner = is_ao_occluder(block_at(snapshot, chunk_x, chunk_z, s1.0, s1.1, s1.2));
    let ao = ao_factor(occ_side1, occ_side2, occ_corner);

    let l0 = light_at(snapshot, chunk_x, chunk_z, base.0, base.1, base.2);
    let l1 = light_at(snapshot, chunk_x, chunk_z, side1[0], side1[1], side1[2]);
    let l2 = light_at(snapshot, chunk_x, chunk_z, side2[0], side2[1], side2[2]);
    let l3 = light_at(snapshot, chunk_x, chunk_z, s1.0, s1.1, s1.2);
    let level = (f32::from(l0.block.max(l0.sky))
        + f32::from(l1.block.max(l1.sky))
        + f32::from(l2.block.max(l2.sky))
        + f32::from(l3.block.max(l3.sky)))
        * 0.25;
    let light = light_factor_from_level(level);
    (ao, light)
}

#[allow(clippy::too_many_arguments)]
fn greedy_face_corner_shades(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    face: Face,
    axis: i32,
    base_y: i32,
    quad: &GreedyQuad,
    block_id: u16,
    voxel_ao_enabled: bool,
    voxel_ao_strength: f32,
    voxel_ao_cutout: bool,
) -> [f32; 4] {
    let x0 = quad.x as i32;
    let x1 = quad.x as i32 + quad.w as i32 - 1;
    let y0 = base_y + quad.y as i32;
    let y1 = base_y + quad.y as i32 + quad.h as i32 - 1;
    let z0 = quad.y as i32;
    let z1 = quad.y as i32 + quad.h as i32 - 1;

    let sample = |sx: i32, sy: i32, sz: i32, vx: f32, vy: f32, vz: f32| {
        compute_vertex_shade(
            snapshot,
            chunk_x,
            chunk_z,
            sx,
            sy,
            sz,
            face,
            [vx, vy, vz],
            block_id,
            voxel_ao_enabled,
            voxel_ao_strength,
            voxel_ao_cutout,
        )
    };

    match face {
        Face::PosY => [
            sample(x0, base_y + axis, z0, 0.0, 1.0, 0.0),
            sample(x1, base_y + axis, z0, 1.0, 1.0, 0.0),
            sample(x1, base_y + axis, z1, 1.0, 1.0, 1.0),
            sample(x0, base_y + axis, z1, 0.0, 1.0, 1.0),
        ],
        Face::NegY => [
            sample(x0, base_y + axis, z1, 0.0, 0.0, 1.0),
            sample(x1, base_y + axis, z1, 1.0, 0.0, 1.0),
            sample(x1, base_y + axis, z0, 1.0, 0.0, 0.0),
            sample(x0, base_y + axis, z0, 0.0, 0.0, 0.0),
        ],
        Face::PosX => [
            sample(axis, y0, x0, 1.0, 0.0, 0.0),
            sample(axis, y0, x1, 1.0, 0.0, 1.0),
            sample(axis, y1, x1, 1.0, 1.0, 1.0),
            sample(axis, y1, x0, 1.0, 1.0, 0.0),
        ],
        Face::NegX => [
            sample(axis, y0, x1, 0.0, 0.0, 1.0),
            sample(axis, y0, x0, 0.0, 0.0, 0.0),
            sample(axis, y1, x0, 0.0, 1.0, 0.0),
            sample(axis, y1, x1, 0.0, 1.0, 1.0),
        ],
        Face::PosZ => [
            sample(x1, y0, axis, 1.0, 0.0, 1.0),
            sample(x0, y0, axis, 0.0, 0.0, 1.0),
            sample(x0, y1, axis, 0.0, 1.0, 1.0),
            sample(x1, y1, axis, 1.0, 1.0, 1.0),
        ],
        Face::NegZ => [
            sample(x0, y0, axis, 0.0, 0.0, 0.0),
            sample(x1, y0, axis, 1.0, 0.0, 0.0),
            sample(x1, y1, axis, 1.0, 1.0, 0.0),
            sample(x0, y1, axis, 0.0, 1.0, 0.0),
        ],
    }
}

fn face_light_factor(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    face: Face,
) -> f32 {
    let (dx, dy, dz) = match face {
        Face::PosX => (1, 0, 0),
        Face::NegX => (-1, 0, 0),
        Face::PosY => (0, 1, 0),
        Face::NegY => (0, -1, 0),
        Face::PosZ => (0, 0, 1),
        Face::NegZ => (0, 0, -1),
    };
    let a = light_at(snapshot, chunk_x, chunk_z, x, y, z);
    let b = light_at(snapshot, chunk_x, chunk_z, x + dx, y + dy, z + dz);
    let level = (f32::from(a.block.max(a.sky)) + f32::from(b.block.max(b.sky))) * 0.5;
    light_factor_from_level(level)
}

fn block_light_factor(
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
) -> f32 {
    let l0 = light_at(snapshot, chunk_x, chunk_z, x, y, z);
    let l1 = light_at(snapshot, chunk_x, chunk_z, x, y + 1, z);
    let level = (f32::from(l0.block.max(l0.sky)) + f32::from(l1.block.max(l1.sky))) * 0.5;
    light_factor_from_level(level)
}

fn block_type(block_state: u16) -> u16 {
    block_state_id(block_state)
}

fn block_meta(block_state: u16) -> u8 {
    block_state_meta(block_state)
}

const fn block_state_from_id(block_id: u16) -> u16 {
    block_id << 4
}

fn is_liquid(block_id: u16) -> bool {
    matches!(block_type(block_id), 8 | 9 | 10 | 11)
}

fn should_apply_prebaked_shade(block_id: u16) -> bool {
    !matches!(
        render_group_for_block(block_id),
        MaterialGroup::Cutout | MaterialGroup::CutoutCulled
    )
}

fn render_group_for_block(block_id: u16) -> MaterialGroup {
    let id = block_type(block_id);
    if is_transparent_block(id) {
        return MaterialGroup::Transparent;
    }
    if is_leaves_block(id) {
        return MaterialGroup::CutoutCulled;
    }
    // Full glass / stained glass blocks should use per-pixel alpha cutout
    // (opaque texels remain opaque; alpha texels are fully discarded).
    if matches!(id, 20 | 95 | 160) {
        return MaterialGroup::CutoutCulled;
    }
    if matches!(
        block_model_kind(block_type(block_id)),
        BlockModelKind::Cross | BlockModelKind::Pane | BlockModelKind::TorchLike
    ) {
        return MaterialGroup::Cutout;
    }
    if matches!(
        id,
        26 | 27 | 28 | 64 | 65 | 66 | 71 | 96 | 157 | 193 | 194 | 195 | 196 | 197
    ) {
        return MaterialGroup::Cutout;
    }
    MaterialGroup::Opaque
}

fn is_occluding_block(block_id: u16) -> bool {
    let id = block_type(block_id);
    if id == 0 {
        return false;
    }
    if is_liquid(block_id) {
        return true;
    }
    if is_alpha_cutout_cube(id) {
        return false;
    }
    !is_custom_block(block_id)
}

fn is_alpha_cutout_cube(id: u16) -> bool {
    is_leaves_block(id) || matches!(id, 20 | 95 | 160)
}

fn fence_connects_to(neighbor_state: u16) -> bool {
    let neighbor_id = block_type(neighbor_state);
    if neighbor_id == 0 || is_liquid(neighbor_state) {
        return false;
    }
    if matches!(block_model_kind(neighbor_id), BlockModelKind::Fence) {
        return true;
    }
    // Fence gates connect visually to fences.
    if matches!(neighbor_id, 107 | 183 | 184 | 185 | 186 | 187) {
        return true;
    }
    is_occluding_block(neighbor_state)
}

fn pane_connects_to(neighbor_state: u16) -> bool {
    let neighbor_id = block_type(neighbor_state);
    if neighbor_id == 0 || is_liquid(neighbor_state) {
        return false;
    }
    if matches!(block_model_kind(neighbor_id), BlockModelKind::Pane) {
        return true;
    }
    // Panes connect to glass-family blocks and iron bars.
    if matches!(neighbor_id, 20 | 95 | 101 | 102 | 160) {
        return true;
    }
    is_occluding_block(neighbor_state)
}

fn wall_connects_to(neighbor_state: u16) -> bool {
    let neighbor_id = block_type(neighbor_state);
    if neighbor_id == 0 || is_liquid(neighbor_state) {
        return false;
    }
    if neighbor_id == 139 {
        return true;
    }
    if matches!(block_model_kind(neighbor_id), BlockModelKind::Fence) {
        return true;
    }
    if matches!(neighbor_id, 107 | 183 | 184 | 185 | 186 | 187) {
        return true;
    }
    is_occluding_block(neighbor_state)
}

fn face_is_occluded(block_id: u16, neighbor_id: u16, leaf_depth_layer_faces: bool) -> bool {
    if block_type(neighbor_id) == 0 {
        return false;
    }
    // Prevent coplanar z-fighting at liquid/solid interfaces:
    // render only the non-liquid face when water/lava touches terrain.
    if is_liquid(block_id) {
        return true;
    }
    if is_liquid(neighbor_id) {
        return is_liquid(block_id);
    }
    // Cull internal faces between identical blend-transparent cubes (e.g. ice).
    let this_type = block_type(block_id);
    let neighbor_type = block_type(neighbor_id);
    if is_transparent_block(this_type) || is_transparent_block(neighbor_type) {
        return this_type == neighbor_type && block_id == neighbor_id;
    }

    // For leaves, keep front faces on deeper leaf blocks so holes in one leaf
    // layer reveal the next layer instead of the sky.
    if leaf_depth_layer_faces && is_leaves_block(this_type) && is_leaves_block(neighbor_type) {
        return false;
    }

    // Alpha-cutout cubes (leaves/glass variants) must not hide solid neighbor faces,
    // otherwise transparent texels show the sky instead of geometry behind.
    // We only cull internal faces between identical cutout cubes.
    if is_alpha_cutout_cube(neighbor_type) {
        return this_type == neighbor_type && block_id == neighbor_id;
    }
    is_occluding_block(neighbor_id)
}
