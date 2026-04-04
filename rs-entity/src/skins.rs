use super::*;
use crate::components::SkinDownloadResult;

fn player_shadow_emissive_strength(player_shadow_opacity: f32) -> LinearRgba {
    // Separate curve from terrain shadows: this keeps skin colors readable without
    // requiring excessively low opacity values.
    let t = 1.0 - player_shadow_opacity.clamp(0.0, 1.0);
    let lift = t * 0.32;
    LinearRgba::rgb(lift, lift, lift)
}

pub(crate) fn skin_download_worker(request_rx: Receiver<String>, result_tx: Sender<SkinDownloadResult>) {
    while let Ok(skin_url) = request_rx.recv() {
        info!("fetching skin: {skin_url}");
        let Ok(response) = reqwest::blocking::get(&skin_url) else {
            warn!("skin fetch failed (request): {skin_url}");
            continue;
        };
        let Ok(bytes) = response.bytes() else {
            warn!("skin fetch failed (bytes): {skin_url}");
            continue;
        };
        let Ok(decoded) = image::load_from_memory(&bytes) else {
            warn!("skin fetch failed (decode): {skin_url}");
            continue;
        };
        let rgba = decoded.to_rgba8();
        let (width, height) = rgba.dimensions();
        info!("skin fetched: {skin_url} ({width}x{height})");
        let _ = result_tx.send(SkinDownloadResult {
            skin_url,
            rgba: rgba.into_raw(),
            width,
            height,
        });
    }
}

pub fn remote_skin_download_tick(
    mut downloader: ResMut<RemoteSkinDownloader>,
    mut images: ResMut<Assets<Image>>,
) {
    while let Ok(downloaded) = downloader.result_rx.try_recv() {
        let mut image = Image::new_fill(
            Extent3d {
                width: downloaded.width,
                height: downloaded.height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0, 0, 0, 0],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        image.data = Some(downloaded.rgba);
        let mut sampler = ImageSamplerDescriptor::nearest();
        sampler.address_mode_u = ImageAddressMode::ClampToEdge;
        sampler.address_mode_v = ImageAddressMode::ClampToEdge;
        image.sampler = ImageSampler::Descriptor(sampler);
        let handle = images.add(image);
        downloader.loaded.insert(downloaded.skin_url, handle);
    }
}

pub fn apply_remote_player_skins(
    registry: Res<RemoteEntityRegistry>,
    downloader: Res<RemoteSkinDownloader>,
    render_debug: Res<RenderDebugSettings>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player_query: Query<(&RemoteEntityUuid, &RemotePlayerSkinMaterials), With<RemotePlayer>>,
) {
    let emissive = player_shadow_emissive_strength(render_debug.player_shadow_opacity);
    for (uuid, player_mats) in &player_query {
        let Some(skin_url) = registry.player_skin_url_by_uuid.get(&uuid.0) else {
            continue;
        };
        let Some(texture_handle) = downloader.skin_handle(skin_url) else {
            continue;
        };
        for mat_handle in &player_mats.0 {
            let Some(material) = materials.get_mut(mat_handle) else {
                continue;
            };
            material.base_color_texture = Some(texture_handle.clone());
            material.emissive_texture = Some(texture_handle.clone());
            material.alpha_mode = AlphaMode::Mask(0.5);
            material.unlit = false;
            material.base_color = Color::WHITE;
            material.emissive = emissive;
        }
    }
}
