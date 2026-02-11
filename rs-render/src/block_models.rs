use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use rs_utils::block_registry_key;
use serde::Deserialize;

use crate::block_textures::Face;

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
}

pub fn default_model_roots() -> Vec<PathBuf> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    vec![
        repo_root.join("rs-client/assets/texturepack/assets"),
        repo_root.join("MavenMCP-1.8.9/src/main/resources/assets"),
        repo_root.join("leafish/resources/assets"),
    ]
}
