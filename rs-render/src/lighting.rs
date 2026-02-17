use bevy::core_pipeline::{
    fxaa::{Fxaa, Sensitivity},
    smaa::{Smaa, SmaaPreset},
};
use bevy::pbr::{
    CascadeShadowConfig, CascadeShadowConfigBuilder, DirectionalLightShadowMap,
    ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel,
};
use bevy::prelude::*;
use bevy::render::view::Msaa;

use crate::chunk::{AtlasLightingUniform, ChunkAtlasMaterial, ChunkRenderAssets};
use crate::components::ShadowCasterLight;
use crate::debug::{AntiAliasingMode, RenderDebugSettings};
use crate::reflection::{DEFAULT_WATER_PLANE_Y, ReflectionPassState};

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

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum ShadowQualityPreset {
    Off,
    Low,
    Medium,
    High,
}

impl Default for ShadowQualityPreset {
    fn default() -> Self {
        Self::Medium
    }
}

impl ShadowQualityPreset {
    pub const ALL: [Self; 4] = [Self::Off, Self::Low, Self::Medium, Self::High];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }

    pub const fn as_options_value(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    pub fn from_options_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "off" => Some(Self::Off),
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ShadowQualityParams {
    map_size: usize,
    cascades: usize,
    max_distance: f32,
    first_cascade_far_bound: f32,
}

fn shadow_quality_params(preset: ShadowQualityPreset) -> ShadowQualityParams {
    match preset {
        ShadowQualityPreset::Off => ShadowQualityParams {
            map_size: 512,
            cascades: 1,
            max_distance: 24.0,
            first_cascade_far_bound: 8.0,
        },
        ShadowQualityPreset::Low => ShadowQualityParams {
            map_size: 1024,
            cascades: 1,
            max_distance: 56.0,
            first_cascade_far_bound: 16.0,
        },
        ShadowQualityPreset::Medium => ShadowQualityParams {
            map_size: 1536,
            cascades: 2,
            max_distance: 96.0,
            first_cascade_far_bound: 28.0,
        },
        ShadowQualityPreset::High => ShadowQualityParams {
            map_size: 2048,
            cascades: 3,
            max_distance: 144.0,
            first_cascade_far_bound: 36.0,
        },
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
            shadow_depth_bias: 0.018,
            shadow_normal_bias: 0.46,
        },
    }
}

pub const fn uses_shadowed_pbr_path(settings: &RenderDebugSettings) -> bool {
    settings.enable_pbr_terrain_lighting
        && matches!(
            settings.lighting_quality,
            LightingQualityPreset::FancyLow | LightingQualityPreset::FancyHigh
        )
}

pub fn lighting_uniform_for(
    settings: &RenderDebugSettings,
    transparent_pass: bool,
) -> AtlasLightingUniform {
    lighting_uniform_for_mode(settings, if transparent_pass { 1.0 } else { 0.0 })
}

pub fn lighting_uniform_for_mode(
    settings: &RenderDebugSettings,
    pass_mode: f32, // 0 opaque, 1 transparent(water), 2 cutout
) -> AtlasLightingUniform {
    let preset = settings.lighting_quality;
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
            pass_mode,
        ),
        color_grading: Vec4::new(
            settings.color_saturation,
            settings.color_contrast,
            settings.color_brightness,
            settings.color_gamma,
        ),
        water_effects: Vec4::new(
            if settings.water_reflections_enabled && settings.water_terrain_ssr {
                2.0
            } else if settings.water_reflections_enabled {
                1.0
            } else {
                0.0
            },
            settings.water_wave_strength,
            settings.water_wave_speed,
            0.0,
        ),
        water_controls: Vec4::new(
            settings.water_reflection_strength,
            DEFAULT_WATER_PLANE_Y,
            settings.water_reflection_near_boost,
            if settings.water_reflection_blue_tint {
                settings.water_reflection_tint_strength
            } else {
                0.0
            },
        ),
        debug_flags: Vec4::new(settings.cutout_debug_mode as f32, 0.0, 0.0, 0.0),
        reflection_view_proj: Mat4::IDENTITY,
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
        mat.extension.lighting = lighting_uniform_for_mode(&settings, 0.0);
        mat.base.unlit = !uses_shadowed_pbr_path(&settings);
        mat.base.alpha_mode = AlphaMode::Opaque;
    }
    if let Some(mat) = materials.get_mut(&assets.cutout_material) {
        mat.extension.lighting = lighting_uniform_for_mode(&settings, 2.0);
        mat.base.unlit = !uses_shadowed_pbr_path(&settings);
        mat.base.alpha_mode = AlphaMode::Mask(0.5);
    }
    if let Some(mat) = materials.get_mut(&assets.cutout_culled_material) {
        mat.extension.lighting = lighting_uniform_for_mode(&settings, 2.0);
        mat.base.unlit = !uses_shadowed_pbr_path(&settings);
        mat.base.alpha_mode = AlphaMode::Mask(0.5);
    }
    if let Some(mat) = materials.get_mut(&assets.transparent_material) {
        mat.extension.lighting = lighting_uniform_for_mode(&settings, 1.0);
        mat.base.unlit = !uses_shadowed_pbr_path(&settings);
        mat.base.alpha_mode = AlphaMode::Blend;
        if settings.water_reflections_enabled {
            mat.base.perceptual_roughness = 0.08;
            mat.base.reflectance = 0.9;
        } else {
            mat.base.perceptual_roughness = 1.0;
            mat.base.reflectance = 0.0;
        }
    }

    let params = preset_params(settings.lighting_quality);
    let shadow_params = shadow_quality_params(settings.shadow_quality);
    let shadow_dist_scale = settings.shadow_distance_scale.clamp(0.25, 20.0);
    shadow_map.size = shadow_params.map_size;

    let allow_shadows =
        settings.shadows_enabled && settings.shadow_quality != ShadowQualityPreset::Off;
    let is_fancy = uses_shadowed_pbr_path(&settings);
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
                    num_cascades: shadow_params.cascades,
                    maximum_distance: shadow_params.max_distance * shadow_dist_scale,
                    first_cascade_far_bound: shadow_params.first_cascade_far_bound * shadow_dist_scale,
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

pub fn update_water_animation(
    time: Res<Time>,
    settings: Res<RenderDebugSettings>,
    assets: Res<ChunkRenderAssets>,
    reflection_state: Option<Res<ReflectionPassState>>,
    mut materials: ResMut<Assets<ChunkAtlasMaterial>>,
) {
    let t = time.elapsed_secs_wrapped();
    let reflection_mode = if settings.water_reflections_enabled && settings.water_terrain_ssr {
        2.0
    } else if settings.water_reflections_enabled {
        1.0
    } else {
        0.0
    };

    let (reflection_view_proj, plane_y) = if let Some(reflection_state) = reflection_state {
        (reflection_state.view_proj, reflection_state.plane_y)
    } else {
        (Mat4::IDENTITY, DEFAULT_WATER_PLANE_Y)
    };

    for handle in [
        &assets.opaque_material,
        &assets.cutout_material,
        &assets.cutout_culled_material,
        &assets.transparent_material,
    ] {
        if let Some(mat) = materials.get_mut(handle) {
            mat.extension.lighting.water_effects = Vec4::new(
                reflection_mode,
                settings.water_wave_strength,
                settings.water_wave_speed,
                t,
            );
            mat.extension.lighting.water_controls =
                Vec4::new(
                    settings.water_reflection_strength,
                    plane_y,
                    settings.water_reflection_near_boost,
                    if settings.water_reflection_blue_tint {
                        settings.water_reflection_tint_strength
                    } else {
                        0.0
                    },
                );
            mat.extension.lighting.reflection_view_proj = reflection_view_proj;
        }
    }
}

pub fn apply_antialiasing(
    settings: Res<RenderDebugSettings>,
    mut camera_query: Query<
        (Entity, Option<&mut Fxaa>, Option<&mut Smaa>, &mut Msaa),
        With<crate::components::PlayerCamera>,
    >,
    mut commands: Commands,
) {
    if !settings.is_changed() {
        return;
    }
    let Ok((camera_entity, fxaa_opt, smaa_opt, mut msaa)) = camera_query.single_mut() else {
        return;
    };

    match settings.aa_mode {
        AntiAliasingMode::Off => {
            *msaa = Msaa::Off;
            if let Some(mut fxaa) = fxaa_opt {
                fxaa.enabled = false;
            }
            if smaa_opt.is_some() {
                commands.entity(camera_entity).remove::<Smaa>();
            }
        }
        AntiAliasingMode::Fxaa => {
            *msaa = Msaa::Off;
            if let Some(mut fxaa) = fxaa_opt {
                fxaa.enabled = true;
                fxaa.edge_threshold = Sensitivity::Ultra;
                fxaa.edge_threshold_min = Sensitivity::High;
            } else {
                commands.entity(camera_entity).insert(Fxaa {
                    enabled: true,
                    edge_threshold: Sensitivity::Ultra,
                    edge_threshold_min: Sensitivity::High,
                });
            }
            if smaa_opt.is_some() {
                commands.entity(camera_entity).remove::<Smaa>();
            }
        }
        AntiAliasingMode::SmaaHigh => {
            *msaa = Msaa::Off;
            if let Some(mut fxaa) = fxaa_opt {
                fxaa.enabled = false;
            }
            if let Some(mut smaa) = smaa_opt {
                smaa.preset = SmaaPreset::High;
            } else {
                commands.entity(camera_entity).insert(Smaa {
                    preset: SmaaPreset::High,
                });
            }
        }
        AntiAliasingMode::SmaaUltra => {
            *msaa = Msaa::Off;
            if let Some(mut fxaa) = fxaa_opt {
                fxaa.enabled = false;
            }
            if let Some(mut smaa) = smaa_opt {
                smaa.preset = SmaaPreset::Ultra;
            } else {
                commands.entity(camera_entity).insert(Smaa {
                    preset: SmaaPreset::Ultra,
                });
            }
        }
        AntiAliasingMode::Msaa4 => {
            *msaa = Msaa::Sample4;
            if let Some(mut fxaa) = fxaa_opt {
                fxaa.enabled = true;
                fxaa.edge_threshold = Sensitivity::High;
                fxaa.edge_threshold_min = Sensitivity::Medium;
            } else {
                commands.entity(camera_entity).insert(Fxaa {
                    enabled: true,
                    edge_threshold: Sensitivity::High,
                    edge_threshold_min: Sensitivity::Medium,
                });
            }
            if smaa_opt.is_some() {
                commands.entity(camera_entity).remove::<Smaa>();
            }
        }
        AntiAliasingMode::Msaa8 => {
            *msaa = Msaa::Sample8;
            if let Some(mut fxaa) = fxaa_opt {
                fxaa.enabled = true;
                fxaa.edge_threshold = Sensitivity::High;
                fxaa.edge_threshold_min = Sensitivity::Medium;
            } else {
                commands.entity(camera_entity).insert(Fxaa {
                    enabled: true,
                    edge_threshold: Sensitivity::High,
                    edge_threshold_min: Sensitivity::Medium,
                });
            }
            if smaa_opt.is_some() {
                commands.entity(camera_entity).remove::<Smaa>();
            }
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
    let aa_uses_msaa = matches!(
        settings.aa_mode,
        AntiAliasingMode::Msaa4 | AntiAliasingMode::Msaa8
    );
    if aa_uses_msaa {
        commands
            .entity(camera_entity)
            .remove::<ScreenSpaceAmbientOcclusion>();
        return;
    }
    match settings.lighting_quality {
        LightingQualityPreset::Fast | LightingQualityPreset::Standard => {
            commands
                .entity(camera_entity)
                .remove::<ScreenSpaceAmbientOcclusion>();
        }
        LightingQualityPreset::FancyLow => {
            commands
                .entity(camera_entity)
                .insert(ScreenSpaceAmbientOcclusion {
                    quality_level: ScreenSpaceAmbientOcclusionQualityLevel::Low,
                    constant_object_thickness: 0.25,
                });
        }
        LightingQualityPreset::FancyHigh => {
            commands
                .entity(camera_entity)
                .insert(ScreenSpaceAmbientOcclusion {
                    quality_level: ScreenSpaceAmbientOcclusionQualityLevel::High,
                    constant_object_thickness: 0.25,
                });
        }
    }
}
