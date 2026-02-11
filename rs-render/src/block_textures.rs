use std::collections::HashMap;
use std::path::Path;

use crate::block_models::BlockModelResolver;
use image::{ImageBuffer, Rgba, imageops};
use rs_utils::{BlockFace as RegistryBlockFace, block_registry_key, block_texture_name};
use rs_utils::{block_state_id, block_state_meta};

pub const ATLAS_COLUMNS: u32 = 64;
pub const ATLAS_ROWS: u32 = 64;
pub const ATLAS_TILE_CAPACITY: usize = (ATLAS_COLUMNS as usize) * (ATLAS_ROWS as usize);

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub enum Face {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

impl Face {
    pub const fn index(self) -> usize {
        match self {
            Self::PosX => 0,
            Self::NegX => 1,
            Self::PosY => 2,
            Self::NegY => 3,
            Self::PosZ => 4,
            Self::NegZ => 5,
        }
    }
}

#[derive(Clone)]
pub struct AtlasBlockMapping {
    face_indices: Vec<[u16; 6]>,
    name_to_index: HashMap<String, u16>,
    pub missing_index: u16,
}

impl AtlasBlockMapping {
    pub fn texture_index(&self, block_id: u16, face: Face) -> u16 {
        self.face_indices
            .get(block_id as usize)
            .map(|arr| arr[face.index()])
            .unwrap_or(self.missing_index)
    }

    pub fn texture_index_for_state(&self, block_state: u16, face: Face) -> u16 {
        let block_id = block_state_id(block_state);
        let meta = block_state_meta(block_state);

        let by_name = |name: &str| -> Option<u16> { self.name_to_index.get(name).copied() };

        let override_name: Option<&'static str> = match block_id {
            5 => Some(match meta & 0x7 {
                1 => "planks_spruce.png",
                2 => "planks_birch.png",
                3 => "planks_jungle.png",
                4 => "planks_acacia.png",
                5 => "planks_big_oak.png",
                _ => "planks_oak.png",
            }),
            6 => Some(match meta & 0x7 {
                1 => "sapling_spruce.png",
                2 => "sapling_birch.png",
                3 => "sapling_jungle.png",
                4 => "sapling_acacia.png",
                5 => "sapling_roofed_oak.png",
                _ => "sapling_oak.png",
            }),
            17 => Some(match face {
                Face::PosY | Face::NegY => match meta & 0x3 {
                    1 => "log_spruce_top.png",
                    2 => "log_birch_top.png",
                    3 => "log_jungle_top.png",
                    _ => "log_oak_top.png",
                },
                _ => match meta & 0x3 {
                    1 => "log_spruce.png",
                    2 => "log_birch.png",
                    3 => "log_jungle.png",
                    _ => "log_oak.png",
                },
            }),
            18 => Some(match meta & 0x3 {
                1 => "leaves_spruce.png",
                2 => "leaves_birch.png",
                3 => "leaves_jungle.png",
                _ => "leaves_oak.png",
            }),
            31 => Some(match meta & 0x3 {
                2 => "fern.png",
                _ => "tallgrass.png",
            }),
            38 => Some(match meta & 0xF {
                1 => "flower_blue_orchid.png",
                2 => "flower_allium.png",
                3 => "flower_houstonia.png",
                4 => "flower_tulip_red.png",
                5 => "flower_tulip_orange.png",
                6 => "flower_tulip_white.png",
                7 => "flower_tulip_pink.png",
                8 => "flower_oxeye_daisy.png",
                _ => "flower_rose.png",
            }),
            161 => Some(match meta & 0x1 {
                1 => "leaves_big_oak.png",
                _ => "leaves_acacia.png",
            }),
            162 => Some(match face {
                Face::PosY | Face::NegY => match meta & 0x1 {
                    1 => "log_big_oak_top.png",
                    _ => "log_acacia_top.png",
                },
                _ => match meta & 0x1 {
                    1 => "log_big_oak.png",
                    _ => "log_acacia.png",
                },
            }),
            175 => {
                let upper = (meta & 0x8) != 0;
                Some(match meta & 0x7 {
                    0 => {
                        if upper {
                            "double_plant_sunflower_top.png"
                        } else {
                            "double_plant_sunflower_bottom.png"
                        }
                    }
                    1 => {
                        if upper {
                            "double_plant_syringa_top.png"
                        } else {
                            "double_plant_syringa_bottom.png"
                        }
                    }
                    2 => "double_plant_grass_bottom.png",
                    3 => "double_plant_fern_bottom.png",
                    4 => {
                        if upper {
                            "double_plant_rose_top.png"
                        } else {
                            "double_plant_rose_bottom.png"
                        }
                    }
                    5 => {
                        if upper {
                            "double_plant_paeonia_top.png"
                        } else {
                            "double_plant_paeonia_bottom.png"
                        }
                    }
                    _ => "double_plant_grass_bottom.png",
                })
            }
            _ => None,
        };

        if let Some(name) = override_name.and_then(by_name) {
            return name;
        }
        self.texture_index(block_id, face)
    }
}

pub fn build_block_texture_mapping(
    name_to_index: &HashMap<String, u16>,
    mut model_resolver: Option<&mut BlockModelResolver>,
) -> AtlasBlockMapping {
    let missing_index = *name_to_index.get("missing_texture.png").unwrap_or(&0);
    let mut face_indices = vec![[missing_index; 6]; 4096];
    let available = name_to_index;

    for block_id in 0u16..=4095u16 {
        for face in [
            Face::PosX,
            Face::NegX,
            Face::PosY,
            Face::NegY,
            Face::PosZ,
            Face::NegZ,
        ] {
            let idx =
                resolve_texture_index(block_id, face, available, model_resolver.as_deref_mut())
                    .unwrap_or(missing_index);
            face_indices[block_id as usize][face.index()] = idx;
        }
    }

    AtlasBlockMapping {
        face_indices,
        name_to_index: available.clone(),
        missing_index,
    }
}

fn resolve_texture_index(
    block_id: u16,
    face: Face,
    available: &HashMap<String, u16>,
    model_resolver: Option<&mut BlockModelResolver>,
) -> Option<u16> {
    if let Some(resolver) = model_resolver {
        if let Some(name) = resolver.face_texture_name(block_id, face) {
            if let Some(idx) = available.get(&name) {
                return Some(*idx);
            }
        }
    }
    for candidate in texture_name_candidates(block_id, face) {
        if let Some(idx) = available.get(&candidate) {
            return Some(*idx);
        }
    }
    None
}

fn texture_name_candidates(block_id: u16, face: Face) -> Vec<String> {
    let mut candidates = Vec::with_capacity(10);
    let registry_face = to_registry_face(face);
    let explicit = block_texture_name(block_id, registry_face).to_string();
    let defer_explicit_stone = explicit == "stone.png" && block_id != 1;
    if !defer_explicit_stone {
        candidates.push(explicit.clone());
    }

    if let Some(registry_key) = block_registry_key(block_id) {
        let base = registry_key
            .strip_prefix("minecraft:")
            .unwrap_or(registry_key);
        // Most block textures in 1.8 are directly keyed by the registry key.
        candidates.push(format!("{base}.png"));

        match face {
            Face::PosY => {
                candidates.push(format!("{base}_top.png"));
                candidates.push(format!("{base}_up.png"));
            }
            Face::NegY => {
                candidates.push(format!("{base}_bottom.png"));
                candidates.push(format!("{base}_down.png"));
            }
            _ => {
                candidates.push(format!("{base}_side.png"));
                candidates.push(format!("{base}_front.png"));
                candidates.push(format!("{base}_end.png"));
            }
        }

        // Common model naming aliases.
        if let Some(trimmed) = base.strip_suffix("_stairs") {
            candidates.push(format!("{trimmed}.png"));
            candidates.push(format!("{trimmed}_side.png"));
        }
        if let Some(trimmed) = base.strip_suffix("_slab") {
            candidates.push(format!("{trimmed}.png"));
            candidates.push(format!("{trimmed}_side.png"));
        }
        if let Some(trimmed) = base.strip_prefix("double_") {
            candidates.push(format!("{trimmed}.png"));
        }
    }

    if defer_explicit_stone {
        candidates.push(explicit);
    }

    dedup_keep_order(candidates)
}

fn dedup_keep_order(input: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(input.len());
    for entry in input {
        if !out.iter().any(|v| v == &entry) {
            out.push(entry);
        }
    }
    out
}

pub fn uv_for_texture() -> [[f32; 2]; 4] {
    // Bevy/WGPU samples with top-left image-space UV convention in this pipeline.
    [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]
}

pub fn atlas_tile_origin(index: u16) -> [f32; 2] {
    let idx = index as u32;
    let col = idx % ATLAS_COLUMNS;
    let row = idx / ATLAS_COLUMNS;
    [
        col as f32 / ATLAS_COLUMNS as f32,
        row as f32 / ATLAS_ROWS as f32,
    ]
}

pub fn is_transparent_block(block_id: u16) -> bool {
    matches!(block_id, 8 | 9 | 10 | 11)
}

pub fn is_water_block(block_id: u16) -> bool {
    matches!(block_id, 8 | 9)
}

pub fn is_grass_block(block_id: u16) -> bool {
    block_id == 2
}

pub fn is_leaves_block(block_id: u16) -> bool {
    matches!(block_id, 18 | 161)
}

fn to_registry_face(face: Face) -> RegistryBlockFace {
    match face {
        Face::PosX => RegistryBlockFace::East,
        Face::NegX => RegistryBlockFace::West,
        Face::PosY => RegistryBlockFace::Up,
        Face::NegY => RegistryBlockFace::Down,
        Face::PosZ => RegistryBlockFace::South,
        Face::NegZ => RegistryBlockFace::North,
    }
}

#[derive(Clone, Copy)]
pub struct BiomeTint {
    pub grass: [f32; 4],
    pub foliage: [f32; 4],
    pub water: [f32; 4],
}

#[derive(Clone, Copy)]
struct BiomeClimate {
    temp: f32,
    rain: f32,
    water: u32,
    grass_override: Option<u32>,
    foliage_override: Option<u32>,
    dark_forest_grass: bool,
}

impl Default for BiomeClimate {
    fn default() -> Self {
        Self {
            temp: 0.5,
            rain: 0.5,
            water: 0xFFFFFF,
            grass_override: None,
            foliage_override: None,
            dark_forest_grass: false,
        }
    }
}

#[derive(Clone)]
pub struct BiomeTintResolver {
    grass_colormap: Vec<u32>,
    foliage_colormap: Vec<u32>,
    climates: [BiomeClimate; 256],
}

impl BiomeTintResolver {
    pub fn load(minecraft_assets_root: &Path) -> Self {
        let grass_colormap = load_colormap(
            &minecraft_assets_root.join("textures/colormap/grass.png"),
            0x7fb238,
        );
        let foliage_colormap = load_colormap(
            &minecraft_assets_root.join("textures/colormap/foliage.png"),
            0x48b518,
        );
        Self {
            grass_colormap,
            foliage_colormap,
            climates: build_biome_climates(),
        }
    }

    pub fn tint_for_biome(&self, biome_id: u8) -> BiomeTint {
        let climate = self.climates[biome_id as usize];
        let mut grass = climate
            .grass_override
            .unwrap_or_else(|| sample_colormap(&self.grass_colormap, climate.temp, climate.rain));
        if climate.dark_forest_grass {
            grass = ((grass & 0xFEFEFE) + 0x28340A) >> 1;
        }
        let foliage = climate
            .foliage_override
            .unwrap_or_else(|| sample_colormap(&self.foliage_colormap, climate.temp, climate.rain));
        BiomeTint {
            grass: rgb_hex(grass),
            foliage: rgb_hex(foliage),
            water: rgb_hex(climate.water),
        }
    }
}

fn rgb(r: f32, g: f32, b: f32) -> [f32; 4] {
    [r, g, b, 1.0]
}

fn rgb_hex(color: u32) -> [f32; 4] {
    rgb(
        ((color >> 16) & 0xFF) as f32 / 255.0,
        ((color >> 8) & 0xFF) as f32 / 255.0,
        (color & 0xFF) as f32 / 255.0,
    )
}

fn load_colormap(path: &Path, fallback: u32) -> Vec<u32> {
    let img = image::open(path)
        .ok()
        .map(|i| i.to_rgba8())
        .unwrap_or_else(|| {
            let [r, g, b, _] = [
                ((fallback >> 16) & 0xFF) as u8,
                ((fallback >> 8) & 0xFF) as u8,
                (fallback & 0xFF) as u8,
                255u8,
            ];
            ImageBuffer::from_pixel(256, 256, Rgba([r, g, b, 255]))
        });
    let img = if img.width() != 256 || img.height() != 256 {
        imageops::resize(&img, 256, 256, imageops::Nearest)
    } else {
        img
    };
    let mut out = vec![fallback; 256 * 256];
    for (i, px) in img.pixels().enumerate() {
        out[i] = ((px[0] as u32) << 16) | ((px[1] as u32) << 8) | px[2] as u32;
    }
    out
}

fn sample_colormap(colormap: &[u32], temp: f32, rain: f32) -> u32 {
    let t = temp.clamp(0.0, 1.0);
    let r = rain.clamp(0.0, 1.0) * t;
    let i = ((1.0 - t) * 255.0) as usize;
    let j = ((1.0 - r) * 255.0) as usize;
    colormap[(j << 8) | i]
}

fn set_climate(
    climates: &mut [BiomeClimate; 256],
    id: usize,
    temp: f32,
    rain: f32,
    water: u32,
    grass_override: Option<u32>,
    foliage_override: Option<u32>,
    dark_forest_grass: bool,
) {
    climates[id] = BiomeClimate {
        temp,
        rain,
        water,
        grass_override,
        foliage_override,
        dark_forest_grass,
    };
}

fn build_biome_climates() -> [BiomeClimate; 256] {
    let mut climates = [BiomeClimate::default(); 256];

    // Decompiled 1.8.9 references:
    // BiomeGenBase#setTemperatureRainfall, ColorizerGrass/Foliage, BiomeColorHelper, BiomeGenSwamp/Mesa/Forest.
    set_climate(&mut climates, 2, 2.0, 0.0, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 3, 0.2, 0.3, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 4, 0.7, 0.8, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 5, 0.25, 0.8, 0xFFFFFF, None, None, false);
    set_climate(
        &mut climates,
        6,
        0.8,
        0.9,
        0xE0FFAE,
        Some(0x6A7039),
        Some(0x6A7039),
        false,
    );
    set_climate(&mut climates, 8, 2.0, 0.0, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 10, 0.0, 0.5, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 11, 0.0, 0.5, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 12, 0.0, 0.5, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 13, 0.0, 0.5, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 14, 0.9, 1.0, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 15, 0.9, 1.0, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 16, 0.8, 0.4, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 17, 2.0, 0.0, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 18, 0.7, 0.8, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 19, 0.25, 0.8, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 20, 0.2, 0.3, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 21, 0.95, 0.9, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 22, 0.95, 0.9, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 23, 0.95, 0.8, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 25, 0.2, 0.3, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 26, 0.05, 0.3, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 27, 0.6, 0.6, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 28, 0.6, 0.6, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 29, 0.7, 0.8, 0xFFFFFF, None, None, true);
    set_climate(&mut climates, 30, -0.5, 0.4, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 31, -0.5, 0.4, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 32, 0.3, 0.8, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 33, 0.3, 0.8, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 34, 0.2, 0.3, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 35, 1.2, 0.0, 0xFFFFFF, None, None, false);
    set_climate(&mut climates, 36, 1.0, 0.0, 0xFFFFFF, None, None, false);
    set_climate(
        &mut climates,
        37,
        2.0,
        0.0,
        0xFFFFFF,
        Some(0x907A45),
        Some(0x9E814D),
        false,
    );
    set_climate(
        &mut climates,
        38,
        2.0,
        0.0,
        0xFFFFFF,
        Some(0x907A45),
        Some(0x9E814D),
        false,
    );
    set_climate(
        &mut climates,
        39,
        2.0,
        0.0,
        0xFFFFFF,
        Some(0x907A45),
        Some(0x9E814D),
        false,
    );

    climates
}

pub fn classify_tint(block_state: u16, block_below_state: Option<u16>) -> TintClass {
    let block_id = block_state_id(block_state);
    match block_id {
        2 | 83 => TintClass::Grass,
        106 => TintClass::Foliage,
        8 | 9 => TintClass::Water,
        18 => {
            let wood = block_state_meta(block_state) & 0x3;
            match wood {
                1 => TintClass::FoliageFixed(0x619961), // spruce
                2 => TintClass::FoliageFixed(0x80A755), // birch
                _ => TintClass::Foliage,
            }
        }
        161 => TintClass::Foliage,
        31 => {
            let kind = block_state_meta(block_state) & 0x3;
            if kind == 0 {
                TintClass::None
            } else {
                TintClass::Grass
            }
        }
        175 => {
            let meta = block_state_meta(block_state);
            let lower_meta = if (meta & 0x8) != 0 {
                block_below_state.map(block_state_meta)
            } else {
                Some(meta)
            };
            if let Some(m) = lower_meta {
                match m & 0x7 {
                    2 | 3 => TintClass::Grass, // double grass / double fern
                    _ => TintClass::None,
                }
            } else {
                TintClass::None
            }
        }
        _ => TintClass::None,
    }
}

#[derive(Clone, Copy)]
pub enum TintClass {
    None,
    Grass,
    Foliage,
    Water,
    FoliageFixed(u32),
}
