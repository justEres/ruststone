use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use rs_utils::block_registry_key;
use rs_utils::ruststone_assets_root;
use serde::Deserialize;

use crate::block_textures::Face;

#[derive(Debug, Clone)]
pub struct IconQuad {
    pub vertices: [[f32; 3]; 4],
    pub uv: [[f32; 2]; 4],
    pub texture_path: String,
    pub tint_index: Option<u8>,
}

#[derive(Default)]
pub struct BlockModelResolver {
    roots: Vec<PathBuf>,
    blockstates: HashMap<String, BlockstateFile>,
    models: HashMap<String, ModelFile>,
    face_cache: HashMap<u16, [Option<String>; 6]>,
}

impl BlockModelResolver {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self {
            roots,
            ..Self::default()
        }
    }

    pub fn face_texture_name(&mut self, block_id: u16, face: Face) -> Option<String> {
        if !self.face_cache.contains_key(&block_id) {
            let resolved = self.resolve_block_face_textures(block_id);
            self.face_cache.insert(block_id, resolved);
        }
        self.face_cache
            .get(&block_id)
            .and_then(|faces| faces[face.index()].clone())
    }

    pub fn icon_quads(&mut self, block_id: u16) -> Option<Vec<IconQuad>> {
        self.icon_quads_for_meta(block_id, 0)
    }

    pub fn icon_quads_for_meta(&mut self, block_id: u16, meta: u8) -> Option<Vec<IconQuad>> {
        let Some(registry_key) = block_registry_key(block_id) else {
            return None;
        };
        let name = registry_key.strip_prefix("minecraft:").unwrap_or(registry_key);
        let blockstate = self.load_blockstate_best(block_id, name, meta)?;
        let model_name = pick_model_name(&blockstate)?;
        let model = self.resolve_model(&model_name, 0)?;

        let mut quads = Vec::new();
        for el in &model.elements {
            let Some(faces) = &el.faces else {
                continue;
            };
            for (dir, face) in faces {
                let Some(texture_path) = resolve_texture_ref_map(&model.textures, &face.texture, 0)
                else {
                    continue;
                };
                let Some(vertices) = face_vertices(el.from, el.to, dir.as_str()) else {
                    continue;
                };
                quads.push(IconQuad {
                    vertices,
                    uv: face_uvs(face),
                    texture_path: append_png(texture_path),
                    tint_index: face.tintindex.map(|v| v as u8),
                });
            }
        }
        Some(quads)
    }

    fn load_blockstate_best(&mut self, block_id: u16, base_name: &str, meta: u8) -> Option<BlockstateFile> {
        for name in blockstate_name_candidates(block_id, base_name, meta) {
            if let Some(state) = self.load_blockstate(&name) {
                return Some(state);
            }
        }
        None
    }

    fn resolve_block_face_textures(&mut self, block_id: u16) -> [Option<String>; 6] {
        let mut out: [Option<String>; 6] = std::array::from_fn(|_| None);
        let Some(registry_key) = block_registry_key(block_id) else {
            return out;
        };
        let name = registry_key
            .strip_prefix("minecraft:")
            .unwrap_or(registry_key);
        let Some(blockstate) = self.load_blockstate(name) else {
            return out;
        };
        let Some(model_name) = pick_model_name(&blockstate) else {
            return out;
        };
        for face in [
            Face::PosX,
            Face::NegX,
            Face::PosY,
            Face::NegY,
            Face::PosZ,
            Face::NegZ,
        ] {
            out[face.index()] = self.resolve_face_from_model(&model_name, face, 0);
        }
        out
    }

    fn resolve_face_from_model(
        &mut self,
        model_name: &str,
        face: Face,
        depth: usize,
    ) -> Option<String> {
        if depth > 16 {
            return None;
        }
        let model = self.load_model(model_name)?.clone();
        if let Some(tex) = model_face_texture_from_elements(&model, face) {
            if let Some(resolved) = resolve_texture_ref(&model, &tex, depth + 1) {
                return Some(append_png(resolved));
            }
        }

        if let Some(parent) = model.parent.as_deref() {
            if let Some(parent_key) = template_texture_key(parent, face) {
                if let Some(tex_ref) = lookup_texture_key(&model, parent_key) {
                    if let Some(resolved) = resolve_texture_ref(&model, &tex_ref, depth + 1) {
                        return Some(append_png(resolved));
                    }
                }
            }
            if let Some(via_parent) = self.resolve_face_from_model(parent, face, depth + 1) {
                return Some(via_parent);
            }
        }
        None
    }

    fn load_blockstate(&mut self, key: &str) -> Option<BlockstateFile> {
        if let Some(cached) = self.blockstates.get(key) {
            return Some(cached.clone());
        }
        let rel = format!("blockstates/{key}.json");
        let raw = self.read_first(&rel)?;
        let parsed = serde_json::from_str::<BlockstateFile>(&raw).ok()?;
        self.blockstates.insert(key.to_string(), parsed.clone());
        Some(parsed)
    }

    fn resolve_model(&mut self, key: &str, depth: usize) -> Option<ResolvedModel> {
        if depth > 24 {
            return None;
        }
        let model = self.load_model(key)?.clone();
        let mut out = if let Some(parent) = model.parent.as_deref() {
            self.resolve_model(parent, depth + 1)?
        } else {
            ResolvedModel::default()
        };
        if let Some(textures) = model.textures {
            for (k, v) in textures {
                out.textures.insert(k, v);
            }
        }
        if let Some(elements) = model.elements {
            out.elements = elements;
        }
        Some(out)
    }

    fn load_model(&mut self, key: &str) -> Option<&ModelFile> {
        if self.models.contains_key(key) {
            return self.models.get(key);
        }
        let model_key = if key.contains(':') {
            key.to_string()
        } else {
            format!("minecraft:{key}")
        };
        let (namespace, path) = split_model_key(&model_key)?;
        let rel = format!("models/{path}.json");
        let raw = self.read_first_in_namespace(namespace, &rel)?;
        let parsed = serde_json::from_str::<ModelFile>(&raw).ok()?;
        self.models.insert(model_key.clone(), parsed);
        self.models.get(&model_key)
    }

    fn read_first(&self, rel: &str) -> Option<String> {
        self.read_first_in_namespace("minecraft", rel)
    }

    fn read_first_in_namespace(&self, namespace: &str, rel: &str) -> Option<String> {
        for root in &self.roots {
            let path = root.join(namespace).join(rel);
            if path.is_file() {
                if let Ok(content) = fs::read_to_string(path) {
                    return Some(content);
                }
            }
        }
        None
    }
}

fn wood_variant(meta: u8) -> &'static str {
    match meta & 0x7 {
        1 => "spruce",
        2 => "birch",
        3 => "jungle",
        4 => "acacia",
        5 => "dark_oak",
        _ => "oak",
    }
}

fn color_variant(meta: u8) -> &'static str {
    match meta & 0xF {
        0 => "white",
        1 => "orange",
        2 => "magenta",
        3 => "light_blue",
        4 => "yellow",
        5 => "lime",
        6 => "pink",
        7 => "gray",
        8 => "silver",
        9 => "cyan",
        10 => "purple",
        11 => "blue",
        12 => "brown",
        13 => "green",
        14 => "red",
        _ => "black",
    }
}

fn blockstate_name_candidates(block_id: u16, base_name: &str, meta: u8) -> Vec<String> {
    let mut out = Vec::with_capacity(6);
    out.push(base_name.to_string());
    match block_id {
        1 => {
            let name = match meta {
                1 => "granite",
                2 => "smooth_granite",
                3 => "diorite",
                4 => "smooth_diorite",
                5 => "andesite",
                6 => "smooth_andesite",
                _ => "stone",
            };
            out.push(name.to_string());
        }
        3 => {
            let name = match meta {
                1 => "coarse_dirt",
                2 => "podzol",
                _ => "dirt",
            };
            out.push(name.to_string());
        }
        5 => out.push(format!("{}_planks", wood_variant(meta))),
        6 => out.push(format!("{}_sapling", wood_variant(meta))),
        12 => out.push(if (meta & 1) == 1 { "red_sand" } else { "sand" }.to_string()),
        17 => out.push(format!(
            "{}_log",
            match meta & 0x3 {
                1 => "spruce",
                2 => "birch",
                3 => "jungle",
                _ => "oak",
            }
        )),
        18 => out.push(format!(
            "{}_leaves",
            match meta & 0x3 {
                1 => "spruce",
                2 => "birch",
                3 => "jungle",
                _ => "oak",
            }
        )),
        24 => {
            let name = match meta & 0x3 {
                1 => "chiseled_sandstone",
                2 => "smooth_sandstone",
                _ => "sandstone",
            };
            out.push(name.to_string());
        }
        35 => out.push(format!("{}_wool", color_variant(meta))),
        38 => {
            let name = match meta {
                1 => "blue_orchid",
                2 => "allium",
                3 => "houstonia",
                4 => "red_tulip",
                5 => "orange_tulip",
                6 => "white_tulip",
                7 => "pink_tulip",
                8 => "oxeye_daisy",
                _ => "poppy",
            };
            out.push(name.to_string());
        }
        43 | 44 => {
            let name = match meta & 0x7 {
                1 => "sandstone_slab",
                2 => "wood_old_slab",
                3 => "cobblestone_slab",
                4 => "brick_slab",
                5 => "stone_brick_slab",
                6 => "nether_brick_slab",
                7 => "quartz_slab",
                _ => "stone_slab",
            };
            out.push(name.to_string());
        }
        95 => out.push(format!("{}_stained_glass", color_variant(meta))),
        97 => {
            let name = match meta {
                1 => "cobblestone",
                2 => "stonebrick",
                3 => "mossy_stonebrick",
                4 => "cracked_stonebrick",
                5 => "chiseled_stonebrick",
                _ => "stone",
            };
            out.push(name.to_string());
        }
        98 => {
            let name = match meta {
                1 => "mossy_stonebrick",
                2 => "cracked_stonebrick",
                3 => "chiseled_stonebrick",
                _ => "stonebrick",
            };
            out.push(name.to_string());
        }
        126 => out.push(format!("{}_slab", wood_variant(meta))),
        159 => out.push(format!("{}_stained_hardened_clay", color_variant(meta))),
        160 => out.push(format!("{}_stained_glass_pane", color_variant(meta))),
        161 => out.push(format!(
            "{}_leaves",
            if (meta & 0x1) == 1 {
                "dark_oak"
            } else {
                "acacia"
            }
        )),
        162 => out.push(format!(
            "{}_log",
            if (meta & 0x1) == 1 {
                "dark_oak"
            } else {
                "acacia"
            }
        )),
        171 => out.push(format!("{}_carpet", color_variant(meta))),
        175 => {
            let name = match meta & 0x7 {
                0 => "sunflower",
                1 => "lilac",
                2 => "double_tallgrass",
                3 => "large_fern",
                4 => "rose_bush",
                5 => "peony",
                _ => "sunflower",
            };
            out.push(name.to_string());
        }
        _ => {}
    }
    dedup_keep_order(out)
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

fn append_png(mut s: String) -> String {
    if !s.ends_with(".png") {
        s.push_str(".png");
    }
    s
}

fn split_model_key(key: &str) -> Option<(&str, &str)> {
    let (namespace, path) = key.split_once(':')?;
    Some((namespace, path))
}

fn pick_model_name(state: &BlockstateFile) -> Option<String> {
    if let Some(variants) = &state.variants {
        if let Some(entry) = variants.get("") {
            return Some(entry.first_model_name());
        }
        if let Some(entry) = variants.get("normal") {
            return Some(entry.first_model_name());
        }
        if let Some((_k, v)) = variants.iter().next() {
            return Some(v.first_model_name());
        }
    }
    if let Some(multipart) = &state.multipart {
        if let Some(part) = multipart.first() {
            return Some(part.apply.first_model_name());
        }
    }
    None
}

fn lookup_texture_key(model: &ModelFile, key: &str) -> Option<String> {
    model
        .textures
        .as_ref()
        .and_then(|map| map.get(key))
        .cloned()
}

fn resolve_texture_ref(model: &ModelFile, tex_ref: &str, depth: usize) -> Option<String> {
    if depth > 16 {
        return None;
    }
    if let Some(key) = tex_ref.strip_prefix('#') {
        let next = lookup_texture_key(model, key)?;
        return resolve_texture_ref(model, &next, depth + 1);
    }
    if let Some(path) = tex_ref.strip_prefix("minecraft:") {
        return Some(path.to_string());
    }
    Some(tex_ref.to_string())
}

fn resolve_texture_ref_map(
    textures: &HashMap<String, String>,
    tex_ref: &str,
    depth: usize,
) -> Option<String> {
    if depth > 24 {
        return None;
    }
    if let Some(key) = tex_ref.strip_prefix('#') {
        let next = textures.get(key)?.clone();
        return resolve_texture_ref_map(textures, &next, depth + 1);
    }
    if let Some(path) = tex_ref.strip_prefix("minecraft:") {
        return Some(path.to_string());
    }
    Some(tex_ref.to_string())
}

fn face_vertices(from: [f32; 3], to: [f32; 3], dir: &str) -> Option<[[f32; 3]; 4]> {
    let (x0, y0, z0) = (from[0] / 16.0, from[1] / 16.0, from[2] / 16.0);
    let (x1, y1, z1) = (to[0] / 16.0, to[1] / 16.0, to[2] / 16.0);
    let verts = match dir {
        "up" => [[x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]],
        "down" => [[x0, y0, z1], [x1, y0, z1], [x1, y0, z0], [x0, y0, z0]],
        "north" => [[x1, y0, z0], [x0, y0, z0], [x0, y1, z0], [x1, y1, z0]],
        "south" => [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
        "west" => [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]],
        "east" => [[x1, y0, z1], [x1, y0, z0], [x1, y1, z0], [x1, y1, z1]],
        _ => return None,
    };
    Some(verts)
}

fn face_uvs(face: &ModelFace) -> [[f32; 2]; 4] {
    if let Some([u0, v0, u1, v1]) = face.uv {
        // Convert to 0..1 UVs.
        let mut uv = [
            [u0 / 16.0, v0 / 16.0],
            [u1 / 16.0, v0 / 16.0],
            [u1 / 16.0, v1 / 16.0],
            [u0 / 16.0, v1 / 16.0],
        ];
        rotate_uvs(&mut uv, face.rotation.unwrap_or(0));
        return uv;
    }
    let mut uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    rotate_uvs(&mut uv, face.rotation.unwrap_or(0));
    uv
}

fn rotate_uvs(uv: &mut [[f32; 2]; 4], rotation: i32) {
    let turns = ((rotation / 90) % 4 + 4) % 4;
    for _ in 0..turns {
        let old = *uv;
        uv[0] = old[3];
        uv[1] = old[0];
        uv[2] = old[1];
        uv[3] = old[2];
    }
}

fn model_face_texture_from_elements(model: &ModelFile, face: Face) -> Option<String> {
    let elements = model.elements.as_ref()?;
    let face_key = match face {
        Face::PosX => "east",
        Face::NegX => "west",
        Face::PosY => "up",
        Face::NegY => "down",
        Face::PosZ => "south",
        Face::NegZ => "north",
    };
    for el in elements {
        if !is_full_cube_element(el) {
            continue;
        }
        if let Some(fd) = el.faces.as_ref().and_then(|faces| faces.get(face_key)) {
            return Some(fd.texture.clone());
        }
    }
    None
}

fn is_full_cube_element(el: &ModelElement) -> bool {
    // Match vanilla cube element bounds.
    el.from == [0.0, 0.0, 0.0] && el.to == [16.0, 16.0, 16.0]
}

fn template_texture_key(parent: &str, face: Face) -> Option<&'static str> {
    let short = parent
        .strip_prefix("minecraft:")
        .unwrap_or(parent)
        .strip_prefix("block/")
        .unwrap_or(parent);
    let key = match short {
        "cube_all" => "all",
        "cube_bottom_top" => match face {
            Face::PosY => "top",
            Face::NegY => "bottom",
            _ => "side",
        },
        "cube_top" => match face {
            Face::PosY => "top",
            Face::NegY => "side",
            _ => "side",
        },
        "cube_column" | "cube_column_horizontal" => match face {
            Face::PosY | Face::NegY => "end",
            _ => "side",
        },
        "cube" => match face {
            Face::PosX => "east",
            Face::NegX => "west",
            Face::PosY => "up",
            Face::NegY => "down",
            Face::PosZ => "south",
            Face::NegZ => "north",
        },
        _ => return None,
    };
    Some(key)
}

#[derive(Debug, Clone, Deserialize)]
struct BlockstateFile {
    #[serde(default)]
    variants: Option<HashMap<String, VariantValue>>,
    #[serde(default)]
    multipart: Option<Vec<MultipartEntry>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum VariantValue {
    One(VariantModel),
    Many(Vec<VariantModel>),
}

impl VariantValue {
    fn first_model_name(&self) -> String {
        match self {
            Self::One(v) => v.model.clone(),
            Self::Many(vs) => vs
                .first()
                .map(|v| v.model.clone())
                .unwrap_or_else(|| "block/stone".to_string()),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct VariantModel {
    model: String,
}

#[derive(Debug, Clone, Deserialize)]
struct MultipartEntry {
    apply: VariantValue,
}

#[derive(Debug, Clone, Deserialize)]
struct ModelFile {
    #[serde(default)]
    parent: Option<String>,
    #[serde(default)]
    textures: Option<HashMap<String, String>>,
    #[serde(default)]
    elements: Option<Vec<ModelElement>>,
}

#[derive(Debug, Clone, Deserialize)]
struct ModelElement {
    from: [f32; 3],
    to: [f32; 3],
    #[serde(default)]
    faces: Option<HashMap<String, ModelFace>>,
}

#[derive(Debug, Clone, Deserialize)]
struct ModelFace {
    texture: String,
    #[serde(default)]
    tintindex: Option<i32>,
    #[serde(default)]
    uv: Option<[f32; 4]>,
    #[serde(default)]
    rotation: Option<i32>,
}

#[derive(Debug, Clone, Default)]
struct ResolvedModel {
    textures: HashMap<String, String>,
    elements: Vec<ModelElement>,
}

pub fn default_model_roots() -> Vec<PathBuf> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut roots = vec![ruststone_assets_root().join("texturepack/assets")];
    let fallback_repo = repo_root.join("rs-client/assets/texturepack/assets");
    if !roots.iter().any(|p| p == &fallback_repo) {
        roots.push(fallback_repo);
    }
    roots
}
