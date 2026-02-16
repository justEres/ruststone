use bevy::pbr::{
    CascadeShadowConfig, CascadeShadowConfigBuilder, DirectionalLightShadowMap,
    ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel,
};
use bevy::prelude::*;
use bevy::render::view::Msaa;

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
    ambient_brightness: f32,
    sun_illuminance: f32,
    fill_illuminance: f32,
    fog_density: f32,
    fog_start: f32,
    fog_end: f32,
    water_absorption: f32,
    water_fresnel: f32,
    shadow_map_size: usize,
    shadow_cascades: usize,
    shadow_max_distance: f32,
    shadow_first_cascade_far_bound: f32,
    shadow_depth_bias: f32,
    shadow_normal_bias: f32,
}

fn preset_params(preset: LightingQualityPreset) -> LightingPresetParams {
    match preset {
        LightingQualityPreset::Fast => LightingPresetParams {
            sun_dir: Vec3::new(0.35, 0.86, 0.36).normalize(),
            sun_strength: 0.0,
            ambient_strength: 1.0,
            ambient_brightness: 1.0,
            sun_illuminance: 6_500.0,
            fill_illuminance: 1_900.0,
            fog_density: 0.0,
            fog_start: 0.0,
            fog_end: 0.0,
            water_absorption: 0.0,
            water_fresnel: 0.0,
            shadow_map_size: 1024,
            shadow_cascades: 1,
            shadow_max_distance: 40.0,
            shadow_first_cascade_far_bound: 12.0,
            shadow_depth_bias: 0.03,
            shadow_normal_bias: 0.8,
        },
        LightingQualityPreset::Standard => LightingPresetParams {
            sun_dir: Vec3::new(0.30, 0.86, 0.42).normalize(),
            sun_strength: 0.48,
            ambient_strength: 0.62,
            ambient_brightness: 0.95,
            sun_illuminance: 7_500.0,
            fill_illuminance: 2_100.0,
            fog_density: 0.0,
            fog_start: 0.0,
            fog_end: 0.0,
            water_absorption: 0.0,
            water_fresnel: 0.0,
            shadow_map_size: 1024,
            shadow_cascades: 1,
            shadow_max_distance: 40.0,
            shadow_first_cascade_far_bound: 12.0,
            shadow_depth_bias: 0.025,
            shadow_normal_bias: 0.7,
        },
        LightingQualityPreset::FancyLow => LightingPresetParams {
            sun_dir: Vec3::new(0.22, 0.88, 0.41).normalize(),
            sun_strength: 0.56,
            ambient_strength: 0.52,
            ambient_brightness: 0.80,
            sun_illuminance: 11_500.0,
            fill_illuminance: 2_200.0,
            fog_density: 0.012,
            fog_start: 70.0,
            fog_end: 220.0,
            water_absorption: 0.18,
            water_fresnel: 0.12,
            shadow_map_size: 1024,
            shadow_cascades: 2,
            shadow_max_distance: 96.0,
            shadow_first_cascade_far_bound: 28.0,
            shadow_depth_bias: 0.022,
            shadow_normal_bias: 0.55,
        },
        LightingQualityPreset::FancyHigh => LightingPresetParams {
            sun_dir: Vec3::new(0.19, 0.90, 0.39).normalize(),
            sun_strength: 0.62,
            ambient_strength: 0.48,
            ambient_brightness: 0.72,
            sun_illuminance: 14_000.0,
            fill_illuminance: 2_450.0,
            fog_density: 0.017,
            fog_start: 52.0,
            fog_end: 170.0,
            water_absorption: 0.26,
            water_fresnel: 0.18,
            shadow_map_size: 2048,
            shadow_cascades: 3,
            shadow_max_distance: 140.0,
            shadow_first_cascade_far_bound: 34.0,
            shadow_depth_bias: 0.018,
            shadow_normal_bias: 0.46,
        },
    }
}

pub const fn uses_shadowed_pbr_path(preset: LightingQualityPreset) -> bool {
    matches!(
        preset,
        LightingQualityPreset::FancyLow | LightingQualityPreset::FancyHigh
    )
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
    mut lights: Query<(
        &mut DirectionalLight,
        &mut Transform,
        Option<&ShadowCasterLight>,
        Option<&mut CascadeShadowConfig>,
    )>,
    mut shadow_map: ResMut<DirectionalLightShadowMap>,
    mut ambient: ResMut<AmbientLight>,
) {
    if !settings.is_changed() {
        return;
    }

    if let Some(mat) = materials.get_mut(&assets.opaque_material) {
        mat.extension.lighting = lighting_uniform_for(settings.lighting_quality, false);
        mat.base.unlit = !uses_shadowed_pbr_path(settings.lighting_quality);
    }
    if let Some(mat) = materials.get_mut(&assets.cutout_material) {
        mat.extension.lighting = lighting_uniform_for(settings.lighting_quality, false);
        mat.base.unlit = !uses_shadowed_pbr_path(settings.lighting_quality);
    }
    if let Some(mat) = materials.get_mut(&assets.cutout_culled_material) {
        mat.extension.lighting = lighting_uniform_for(settings.lighting_quality, false);
        mat.base.unlit = !uses_shadowed_pbr_path(settings.lighting_quality);
    }
    if let Some(mat) = materials.get_mut(&assets.transparent_material) {
        mat.extension.lighting = lighting_uniform_for(settings.lighting_quality, true);
        mat.base.unlit = !uses_shadowed_pbr_path(settings.lighting_quality);
    }

    let params = preset_params(settings.lighting_quality);
    shadow_map.size = params.shadow_map_size;

    let allow_shadows =
        settings.shadows_enabled && uses_shadowed_pbr_path(settings.lighting_quality);
    let is_fancy = uses_shadowed_pbr_path(settings.lighting_quality);
    let sun_travel_dir = -params.sun_dir;
    ambient.brightness = params.ambient_brightness;

    for (mut light, mut light_transform, shadow_light, cascade_cfg) in &mut lights {
        if shadow_light.is_some() {
            light.shadows_enabled = allow_shadows;
            light.illuminance = params.sun_illuminance;
            light.shadow_depth_bias = params.shadow_depth_bias;
            light.shadow_normal_bias = params.shadow_normal_bias;
            light_transform.look_to(sun_travel_dir, Vec3::Y);
            if let Some(mut cascade_cfg) = cascade_cfg {
                *cascade_cfg = CascadeShadowConfigBuilder {
                    num_cascades: params.shadow_cascades,
                    maximum_distance: params.shadow_max_distance,
                    first_cascade_far_bound: params.shadow_first_cascade_far_bound,
                    minimum_distance: 0.1,
                    ..default()
                }
                .into();
            }
        } else {
            light.illuminance = if is_fancy {
                params.fill_illuminance
            } else {
                params.fill_illuminance * 0.95
            };
            light_transform.look_to(
                Vec3::new(-sun_travel_dir.x, sun_travel_dir.y, -sun_travel_dir.z),
                Vec3::Y,
            );
        }
    }
}

pub fn apply_ssao_quality(
    settings: Res<RenderDebugSettings>,
    camera_query: Query<Entity, With<crate::components::PlayerCamera>>,
    mut commands: Commands,
) {
    if !settings.is_changed() {
        return;
    }
    let Ok(camera_entity) = camera_query.single() else {
        return;
    };
    match settings.lighting_quality {
        LightingQualityPreset::Fast | LightingQualityPreset::Standard => {
            commands
                .entity(camera_entity)
                .insert(Msaa::Sample4)
                .remove::<ScreenSpaceAmbientOcclusion>();
        }
        LightingQualityPreset::FancyLow => {
            commands
                .entity(camera_entity)
                .insert(Msaa::Off)
                .insert(ScreenSpaceAmbientOcclusion {
                    quality_level: ScreenSpaceAmbientOcclusionQualityLevel::Low,
                    constant_object_thickness: 0.25,
                });
        }
        LightingQualityPreset::FancyHigh => {
            commands
                .entity(camera_entity)
                .insert(Msaa::Off)
                .insert(ScreenSpaceAmbientOcclusion {
                    quality_level: ScreenSpaceAmbientOcclusionQualityLevel::High,
                    constant_object_thickness: 0.25,
                });
        }
    }
}
