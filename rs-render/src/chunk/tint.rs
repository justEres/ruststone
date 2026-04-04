use super::*;

pub(super) fn apply_default_foliage_tint(
    texture_name: &str,
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
) {
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

pub(super) fn neutralize_grass_side_base_with_overlay_mask(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    overlay: &ImageBuffer<Rgba<u8>, Vec<u8>>,
) {
    if img.dimensions() != overlay.dimensions() {
        return;
    }

    for (base, mask_px) in img.pixels_mut().zip(overlay.pixels()) {
        if base.0[3] == 0 {
            continue;
        }
        let [r, g, b, a] = mask_px.0;
        let mask = if a == 255 {
            ((u16::from(r) + u16::from(g) + u16::from(b)) / 3) as u8
        } else {
            a
        };
        if mask == 0 {
            continue;
        }
        let luma =
            ((u16::from(base.0[0]) * 54 + u16::from(base.0[1]) * 183 + u16::from(base.0[2]) * 19)
                / 256) as u8;
        let blend = u16::from(mask);
        let inv = 255_u16.saturating_sub(blend);
        base.0[0] = ((u16::from(base.0[0]) * inv + u16::from(luma) * blend) / 255) as u8;
        base.0[1] = ((u16::from(base.0[1]) * inv + u16::from(luma) * blend) / 255) as u8;
        base.0[2] = ((u16::from(base.0[2]) * inv + u16::from(luma) * blend) / 255) as u8;
    }
}

pub(super) fn normalize_overlay_mask_texture(
    texture_name: &str,
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
) {
    if !texture_name.ends_with("_overlay.png") {
        return;
    }
    for p in img.pixels_mut() {
        let [r, g, b, a] = p.0;
        let luma = ((u16::from(r) + u16::from(g) + u16::from(b)) / 3) as u8;
        let mask = if a == 255 { luma } else { a };
        p.0 = [255, 255, 255, mask];
    }
}

pub(super) fn force_opaque_texture_alpha(
    texture_name: &str,
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
) {
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

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_foliage_tint_strength(color: [f32; 4], strength: f32) -> [f32; 4] {
    let strength = strength.clamp(0.0, 2.5);
    if strength <= 1.0 {
        return [
            1.0 + (color[0] - 1.0) * strength,
            1.0 + (color[1] - 1.0) * strength,
            1.0 + (color[2] - 1.0) * strength,
            color[3],
        ];
    }

    let extra = strength - 1.0;
    let luma = color[0] * 0.2126 + color[1] * 0.7152 + color[2] * 0.0722;
    let sat_scale = 1.0 + extra * 0.85;
    let lift = extra * 0.10;
    [
        (luma + (color[0] - luma) * sat_scale + (1.0 - color[0]) * lift).clamp(0.0, 1.0),
        (luma + (color[1] - luma) * sat_scale + (1.0 - color[1]) * lift).clamp(0.0, 1.0),
        (luma + (color[2] - luma) * sat_scale + (1.0 - color[2]) * lift).clamp(0.0, 1.0),
        color[3],
    ]
}

pub(super) fn apply_runtime_biome_tint(
    block_id: u16,
    below: Option<u16>,
    color: [f32; 4],
    vanilla_bake: Option<VanillaBakeSettings>,
) -> [f32; 4] {
    match (classify_tint(block_id, below), vanilla_bake) {
        (TintClass::Foliage, Some(vanilla_bake)) => {
            apply_foliage_tint_strength(color, vanilla_bake.foliage_tint_strength)
        }
        _ => color,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn tint_color(
    block_id: u16,
    tint: BiomeTint,
    snapshot: &ChunkColumnSnapshot,
    chunk_x: i32,
    chunk_z: i32,
    x: i32,
    y: i32,
    z: i32,
    biome_tints: &BiomeTintResolver,
    vanilla_bake: Option<VanillaBakeSettings>,
) -> [f32; 4] {
    let below = if block_type(block_id) == 175 {
        Some(block_at(snapshot, chunk_x, chunk_z, x, y - 1, z))
    } else {
        None
    };
    let color = match classify_tint(block_id, below) {
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
    };
    apply_runtime_biome_tint(block_id, below, color, vanilla_bake)
}

pub(super) fn tint_color_untargeted(
    block_id: u16,
    tint: BiomeTint,
    vanilla_bake: Option<VanillaBakeSettings>,
) -> [f32; 4] {
    let color = match classify_tint(block_id, None) {
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
    };
    apply_runtime_biome_tint(block_id, None, color, vanilla_bake)
}
