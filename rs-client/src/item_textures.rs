use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::thread;

use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use crossbeam::channel::{Receiver, Sender, unbounded};
use rs_utils::{InventoryItemStack, item_texture_candidates, texturepack_textures_root};
use tracing::warn;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ItemTexKey {
    pub item_id: i32,
    pub damage: i16,
}

#[derive(Debug)]
struct ItemTexResult {
    key: ItemTexKey,
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

#[derive(Resource)]
pub struct ItemTextureCache {
    request_tx: Sender<ItemTexKey>,
    result_rx: Receiver<ItemTexResult>,
    requested: HashSet<ItemTexKey>,
    loaded: HashMap<ItemTexKey, Handle<Image>>,
    materials: HashMap<ItemTexKey, Handle<StandardMaterial>>,
}

#[derive(Resource, Clone)]
pub struct ItemSpriteMesh(pub Handle<Mesh>);

pub fn init_item_sprite_mesh(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let handle = meshes.add(build_item_quad_mesh());
    commands.insert_resource(ItemSpriteMesh(handle));
}

impl Default for ItemTextureCache {
    fn default() -> Self {
        let (request_tx, request_rx) = unbounded::<ItemTexKey>();
        let (result_tx, result_rx) = unbounded::<ItemTexResult>();
        thread::spawn(move || item_texture_worker(request_rx, result_tx));
        Self {
            request_tx,
            result_rx,
            requested: HashSet::new(),
            loaded: HashMap::new(),
            materials: HashMap::new(),
        }
    }
}

impl ItemTextureCache {
    pub fn request_stack(&mut self, stack: &InventoryItemStack) {
        let key = ItemTexKey {
            item_id: stack.item_id,
            damage: stack.damage,
        };
        if self.requested.insert(key) {
            let _ = self.request_tx.send(key);
        }
        // Also request damage-0 fallback so we have something if subtype textures are missing.
        if stack.damage != 0 {
            let base = ItemTexKey {
                item_id: stack.item_id,
                damage: 0,
            };
            if self.requested.insert(base) {
                let _ = self.request_tx.send(base);
            }
        }
    }

    pub fn texture_for_stack(&self, stack: &InventoryItemStack) -> Option<Handle<Image>> {
        let key = ItemTexKey {
            item_id: stack.item_id,
            damage: stack.damage,
        };
        if let Some(h) = self.loaded.get(&key) {
            return Some(h.clone());
        }
        if stack.damage != 0 {
            let base = ItemTexKey {
                item_id: stack.item_id,
                damage: 0,
            };
            if let Some(h) = self.loaded.get(&base) {
                return Some(h.clone());
            }
        }
        None
    }

    pub fn material_for_stack(
        &self,
        stack: &InventoryItemStack,
    ) -> Option<Handle<StandardMaterial>> {
        let key = ItemTexKey {
            item_id: stack.item_id,
            damage: stack.damage,
        };
        if let Some(h) = self.materials.get(&key) {
            return Some(h.clone());
        }
        if stack.damage != 0 {
            let base = ItemTexKey {
                item_id: stack.item_id,
                damage: 0,
            };
            if let Some(h) = self.materials.get(&base) {
                return Some(h.clone());
            }
        }
        None
    }

    pub fn insert_material(&mut self, key: ItemTexKey, handle: Handle<StandardMaterial>) {
        self.materials.insert(key, handle);
    }
}

pub fn item_texture_cache_tick(
    mut cache: ResMut<ItemTextureCache>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    while let Ok(result) = cache.result_rx.try_recv() {
        let mut image = Image::new_fill(
            Extent3d {
                width: result.width,
                height: result.height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0, 0, 0, 0],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        image.data = Some(result.rgba);

        let mut sampler = ImageSamplerDescriptor::nearest();
        sampler.address_mode_u = ImageAddressMode::ClampToEdge;
        sampler.address_mode_v = ImageAddressMode::ClampToEdge;
        sampler.address_mode_w = ImageAddressMode::ClampToEdge;
        image.sampler = ImageSampler::Descriptor(sampler);

        let tex_handle = images.add(image);
        cache.loaded.insert(result.key, tex_handle.clone());

        let mat_handle = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(tex_handle),
            // Items (potions, etc.) often use semi-transparent pixels. Use blending instead of cutout.
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            unlit: true,
            perceptual_roughness: 1.0,
            metallic: 0.0,
            ..Default::default()
        });
        cache.insert_material(result.key, mat_handle);
    }
}

fn item_texture_worker(request_rx: Receiver<ItemTexKey>, result_tx: Sender<ItemTexResult>) {
    let root = textures_root();
    while let Ok(key) = request_rx.recv() {
        let candidates = item_texture_candidates(key.item_id, key.damage);
        let mut found: Option<PathBuf> = None;
        for rel in candidates {
            let p = root.join(rel);
            if p.is_file() {
                found = Some(p);
                break;
            }
        }

        let rgba = if let Some(path) = found {
            match std::fs::read(&path)
                .ok()
                .and_then(|bytes| image::load_from_memory(&bytes).ok())
            {
                Some(img) => {
                    let rgba = img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    let _ = result_tx.send(ItemTexResult {
                        key,
                        rgba: rgba.into_raw(),
                        width: w,
                        height: h,
                    });
                    continue;
                }
                None => {
                    warn!("failed to decode item texture for {:?} at {:?}", key, path);
                    None
                }
            }
        } else {
            None
        };

        let (w, h, data) = missing_texture_rgba();
        let _ = result_tx.send(ItemTexResult {
            key,
            rgba: rgba.unwrap_or_else(|| data),
            width: w,
            height: h,
        });
    }
}

fn textures_root() -> PathBuf {
    texturepack_textures_root()
}

fn missing_texture_rgba() -> (u32, u32, Vec<u8>) {
    let w = 16u32;
    let h = 16u32;
    let mut out = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            let is_dark = ((x + y) & 1) == 0;
            let (r, g, b) = if is_dark { (0, 0, 0) } else { (255, 0, 255) };
            out[idx] = r;
            out[idx + 1] = g;
            out[idx + 2] = b;
            out[idx + 3] = 255;
        }
    }
    (w, h, out)
}

fn build_item_quad_mesh() -> Mesh {
    use bevy::render::mesh::Indices;
    use bevy::render::mesh::PrimitiveTopology;

    // Quad in the XY plane, normal pointing +Z.
    // UVs use the same "V flipped" convention as the rest of the renderer.
    let positions = vec![
        [-0.5, -0.5, 0.0],
        [0.5, -0.5, 0.0],
        [0.5, 0.5, 0.0],
        [-0.5, 0.5, 0.0],
    ];
    let normals = vec![[0.0, 0.0, 1.0]; 4];
    // Rotate sprite UVs 90° so item textures render vertically.
    let uvs = vec![[1.0, 1.0], [1.0, 0.0], [0.0, 0.0], [0.0, 1.0]];
    let indices = vec![0u32, 1, 2, 0, 2, 3];

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}
