use std::collections::HashMap;

use crate::block_models::BlockModelResolver;
use rs_utils::{BlockFace as RegistryBlockFace, block_registry_key, block_texture_name};

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
    pub missing_index: u16,
}

impl AtlasBlockMapping {
    pub fn texture_index(&self, block_id: u16, face: Face) -> u16 {
        self.face_indices
            .get(block_id as usize)
            .map(|arr| arr[face.index()])
            .unwrap_or(self.missing_index)
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

pub fn biome_tint(biome_id: u8) -> BiomeTint {
    match biome_id {
        2 | 17 => BiomeTint {
            grass: rgb(0.91, 0.77, 0.38),
            foliage: rgb(0.85, 0.74, 0.4),
            water: rgb(0.25, 0.42, 0.8),
        },
        6 => BiomeTint {
            grass: rgb(0.4, 0.56, 0.2),
            foliage: rgb(0.35, 0.5, 0.2),
            water: rgb(0.2, 0.36, 0.5),
        },
        5 | 19 | 20 | 30 | 31 => BiomeTint {
            grass: rgb(0.5, 0.6, 0.5),
            foliage: rgb(0.45, 0.55, 0.45),
            water: rgb(0.25, 0.42, 0.8),
        },
        8 | 9 => BiomeTint {
            grass: rgb(0.3, 0.3, 0.3),
            foliage: rgb(0.25, 0.25, 0.25),
            water: rgb(0.4, 0.1, 0.1),
        },
        12 | 140 => BiomeTint {
            grass: rgb(0.8, 0.8, 0.9),
            foliage: rgb(0.8, 0.8, 0.9),
            water: rgb(0.25, 0.42, 0.8),
        },
        21 | 22 | 23 => BiomeTint {
            grass: rgb(0.2, 0.6, 0.2),
            foliage: rgb(0.2, 0.55, 0.2),
            water: rgb(0.25, 0.42, 0.8),
        },
        35 | 36 => BiomeTint {
            grass: rgb(0.5, 0.7, 0.2),
            foliage: rgb(0.45, 0.65, 0.2),
            water: rgb(0.25, 0.42, 0.8),
        },
        37 | 38 | 39 => BiomeTint {
            grass: rgb(0.75, 0.65, 0.4),
            foliage: rgb(0.6, 0.55, 0.35),
            water: rgb(0.25, 0.42, 0.8),
        },
        _ => BiomeTint {
            grass: rgb(0.36, 0.74, 0.29),
            foliage: rgb(0.28, 0.7, 0.22),
            water: rgb(0.25, 0.42, 0.8),
        },
    }
}

fn rgb(r: f32, g: f32, b: f32) -> [f32; 4] {
    [r, g, b, 1.0]
}
