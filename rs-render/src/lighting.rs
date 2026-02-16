use bevy::prelude::*;

use crate::chunk::{AtlasLightingUniform, ChunkAtlasMaterial, ChunkRenderAssets};
use crate::components::ShadowCasterLight;
use crate::debug::RenderDebugSettings;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum LightingQualityPreset {
    Fast,
    Standard,
    FancyLow,
    FancyHigh,
}

impl Default for LightingQualityPreset {
    fn default() -> Self {
        Self::Standard
    }
}

impl LightingQualityPreset {
    pub const ALL: [Self; 4] = [Self::Fast, Self::Standard, Self::FancyLow, Self::FancyHigh];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Fast => "Fast",
            Self::Standard => "Standard",
            Self::FancyLow => "Fancy Low",
            Self::FancyHigh => "Fancy High",
        }
    }

    pub const fn as_options_value(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Standard => "standard",
            Self::FancyLow => "fancy_low",
            Self::FancyHigh => "fancy_high",
        }
    }

    pub fn from_options_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "fast" => Some(Self::Fast),
            "standard" => Some(Self::Standard),
            "fancy_low" | "fancylow" => Some(Self::FancyLow),
            "fancy_high" | "fancyhigh" => Some(Self::FancyHigh),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct LightingPresetParams {
    sun_dir: Vec3,
    sun_strength: f32,
    ambient_strength: f32,
    fog_density: f32,
    fog_start: f32,
    fog_end: f32,
    water_absorption: f32,
    water_fresnel: f32,
}

fn preset_params(preset: LightingQualityPreset) -> LightingPresetParams {
    match preset {
        LightingQualityPreset::Fast => LightingPresetParams {
            sun_dir: Vec3::new(0.35, 0.86, 0.36).normalize(),
            sun_strength: 0.0,
            ambient_strength: 1.0,
            fog_density: 0.0,
            fog_start: 0.0,
            fog_end: 0.0,
            water_absorption: 0.0,
            water_fresnel: 0.0,
        },
        LightingQualityPreset::Standard => LightingPresetParams {
            sun_dir: Vec3::new(0.30, 0.86, 0.42).normalize(),
            sun_strength: 0.48,
            ambient_strength: 0.62,
            fog_density: 0.0,
            fog_start: 0.0,
            fog_end: 0.0,
            water_absorption: 0.0,
            water_fresnel: 0.0,
        },
        LightingQualityPreset::FancyLow => LightingPresetParams {
            sun_dir: Vec3::new(0.22, 0.88, 0.41).normalize(),
            sun_strength: 0.56,
            ambient_strength: 0.52,
            fog_density: 0.012,
            fog_start: 70.0,
            fog_end: 220.0,
            water_absorption: 0.18,
            water_fresnel: 0.12,
        },
        LightingQualityPreset::FancyHigh => LightingPresetParams {
            sun_dir: Vec3::new(0.19, 0.90, 0.39).normalize(),
            sun_strength: 0.62,
            ambient_strength: 0.48,
            fog_density: 0.017,
            fog_start: 52.0,
            fog_end: 170.0,
            water_absorption: 0.26,
            water_fresnel: 0.18,
        },
    }
}

pub fn lighting_uniform_for(
    preset: LightingQualityPreset,
    transparent_pass: bool,
) -> AtlasLightingUniform {
    let params = preset_params(preset);
    AtlasLightingUniform {
        sun_dir_and_strength: Vec4::new(
            params.sun_dir.x,
            params.sun_dir.y,
            params.sun_dir.z,
            params.sun_strength,
        ),
        ambient_and_fog: Vec4::new(
            params.ambient_strength,
            params.fog_density,
            params.fog_start,
            params.fog_end,
        ),
        quality_and_water: Vec4::new(
            preset as u32 as f32,
            params.water_absorption,
            params.water_fresnel,
            if transparent_pass { 1.0 } else { 0.0 },
        ),
    }
}

pub fn apply_lighting_quality(
    settings: Res<RenderDebugSettings>,
    assets: Res<ChunkRenderAssets>,
    mut materials: ResMut<Assets<ChunkAtlasMaterial>>,
    mut lights: Query<(&mut DirectionalLight, Option<&ShadowCasterLight>)>,
) {
    if !settings.is_changed() {
        return;
    }

    if let Some(mat) = materials.get_mut(&assets.opaque_material) {
        mat.extension.lighting = lighting_uniform_for(settings.lighting_quality, false);
    }
    if let Some(mat) = materials.get_mut(&assets.cutout_material) {
        mat.extension.lighting = lighting_uniform_for(settings.lighting_quality, false);
    }
    if let Some(mat) = materials.get_mut(&assets.cutout_culled_material) {
        mat.extension.lighting = lighting_uniform_for(settings.lighting_quality, false);
    }
    if let Some(mat) = materials.get_mut(&assets.transparent_material) {
        mat.extension.lighting = lighting_uniform_for(settings.lighting_quality, true);
    }

    let allow_shadows = settings.shadows_enabled
        && matches!(
            settings.lighting_quality,
            LightingQualityPreset::FancyLow | LightingQualityPreset::FancyHigh
        );
    let is_fancy = matches!(
        settings.lighting_quality,
        LightingQualityPreset::FancyLow | LightingQualityPreset::FancyHigh
    );

    for (mut light, shadow_light) in &mut lights {
        if shadow_light.is_some() {
            light.shadows_enabled = allow_shadows;
            light.illuminance = if is_fancy { 9_000.0 } else { 7_000.0 };
        } else {
            light.illuminance = if is_fancy { 2_300.0 } else { 2_000.0 };
        }
    }
}
