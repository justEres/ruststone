use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::thread;

use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use crossbeam::channel::{Receiver, Sender, unbounded};
use tracing::warn;

#[derive(Component, Debug, Clone, Copy)]
pub struct EntityTexturePath(pub &'static str);

#[derive(Debug)]
struct TextureResult {
    path: &'static str,
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

#[derive(Resource)]
pub struct EntityTextureCache {
    request_tx: Sender<&'static str>,
    result_rx: Receiver<TextureResult>,
    requested: HashSet<&'static str>,
    loaded: HashMap<&'static str, Handle<Image>>,
    materials: HashMap<&'static str, Handle<StandardMaterial>>,
}

impl Default for EntityTextureCache {
    fn default() -> Self {
        let (request_tx, request_rx) = unbounded::<&'static str>();
        let (result_tx, result_rx) = unbounded::<TextureResult>();
        thread::spawn(move || texture_worker(request_rx, result_tx));
        Self {
            request_tx,
            result_rx,
            requested: HashSet::new(),
            loaded: HashMap::new(),
            materials: HashMap::new(),
        }
    }
}

impl EntityTextureCache {
    pub fn request(&mut self, path: &'static str) {
        if self.requested.insert(path) {
            let _ = self.request_tx.send(path);
        }
    }

    pub fn material(&self, path: &'static str) -> Option<Handle<StandardMaterial>> {
        self.materials.get(path).cloned()
    }
}

pub fn entity_texture_cache_tick(
    mut cache: ResMut<EntityTextureCache>,
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
        cache.loaded.insert(result.path, tex_handle.clone());

        let mat_handle = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(tex_handle),
            // Many entity textures use 0-alpha pixels for cutouts (eyes, fur, etc.).
            alpha_mode: AlphaMode::Mask(0.5),
            unlit: true,
            perceptual_roughness: 1.0,
            metallic: 0.0,
            ..Default::default()
        });
        cache.materials.insert(result.path, mat_handle);
    }
}

fn texture_worker(request_rx: Receiver<&'static str>, result_tx: Sender<TextureResult>) {
    let root = textures_root();
    while let Ok(path) = request_rx.recv() {
        let full = root.join(path);
        let Some(decoded) = std::fs::read(&full)
            .ok()
            .and_then(|bytes| image::load_from_memory(&bytes).ok())
        else {
            warn!("failed to load entity texture: {:?}", full);
            continue;
        };
        let rgba = decoded.to_rgba8();
        let (width, height) = rgba.dimensions();
        let _ = result_tx.send(TextureResult {
            path,
            rgba: rgba.into_raw(),
            width,
            height,
        });
    }
}

fn textures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../rs-client/assets/texturepack/assets/minecraft/textures")
}
